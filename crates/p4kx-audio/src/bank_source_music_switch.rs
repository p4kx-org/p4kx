// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc::CONTAINER_VERSION;
use crate::hirc_music_node::{self, MusicNodeLists};
use crate::hirc_music_segment;
use crate::hirc_music_switch::{self, BankMusicSwitchFields, DecisionTree, SwitchGroups};
use crate::hirc_music_switch_traversal::Traversal;
use crate::hirc_music_transition;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError, HircKind};

#[path = "bank_source_music_switch_rows.rs"]
mod rows;

pub use self::rows::{BankMusicSwitchRow, BankMusicSwitchRows};

/// A physical music switch and its checked original fields and rows.
#[non_exhaustive]
pub struct BankHircMusicSwitch<'input, R> {
    /// Original object envelope.
    pub object: BankHircObject,
    /// Interpretation status of the selected layout.
    pub status: BankMusicSwitchRead,
    /// Successfully observed original fields.
    pub fields: BankMusicSwitchFields,
    /// Bounded replay of original lists and admitted targets.
    pub rows: BankMusicSwitchRows<'input, R>,
}

/// Interpretation status of a physical music switch.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicSwitchRead {
    /// Existing-reader fields and tree fit the body.
    Decoded,
    /// The selected layout failed after a readable prefix.
    Malformed(BankError),
    /// The bank version has no known music switch layout.
    Unsupported(BankError),
}

#[derive(Default)]
struct SwitchLists {
    node: MusicNodeLists,
    groups: SwitchGroups,
    tree: Option<DecisionTree>,
    targets: u32,
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircMusicSwitch<'input, R>, BankReadError> {
    let mut fields = BankMusicSwitchFields::default();
    let mut lists = SwitchLists::default();
    let status = if version == CONTAINER_VERSION {
        match scan(input, &object, &mut fields, &mut lists) {
            Ok(()) => BankMusicSwitchRead::Decoded,
            Err(BankReadError::Format(error)) => BankMusicSwitchRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    } else {
        BankMusicSwitchRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::MusicSwitch,
            version,
        })
    };
    Ok(BankHircMusicSwitch {
        object,
        status,
        fields,
        rows: BankMusicSwitchRows::new(input, stopped, lists),
    })
}

fn scan<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: &BankHircObject,
    fields: &mut BankMusicSwitchFields,
    lists: &mut SwitchLists,
) -> Result<(), BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    hirc_music_node::scan(&mut reader, &mut fields.node, &mut lists.node)?;
    hirc_music_transition::read(&mut reader, &mut fields.transitions)?;
    let tree = hirc_music_switch::header(&mut reader, fields, &mut lists.groups, |_| {})?;
    let available = u32::try_from(reader.remaining_bytes())
        .unwrap_or(u32::MAX)
        .min(tree.byte_len);
    let admitted = DecisionTree {
        byte_len: available,
        ..tree
    };
    fields.tree_range = Some(admitted.range());
    lists.tree = Some(admitted);
    reader.skip_bytes(u64::from(tree.byte_len))?;
    let finish = hirc_music_segment::finish(&reader);
    let mut traversal = Traversal::new(tree.checked_nodes()?, tree.depth);
    while traversal
        .next_target(|index| rows::tree_node(input, tree, index))?
        .is_some()
    {
        lists.targets += 1;
    }
    finish
}
