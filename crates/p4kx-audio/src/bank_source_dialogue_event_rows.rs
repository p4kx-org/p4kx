// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::bank_source_music_switch::read_group;
use crate::hirc_dialogue_event::BankDialogueTree;
use crate::hirc_music_switch::{self, NODE_LEN, SwitchGroups};
use crate::read_at::SeekSource;

/// One original `DialogueEvent` argument or physical node occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankDialogueEventRow {
    /// Original group ID and its separately checked type byte.
    Argument {
        /// Original argument position.
        ordinal: u32,
        /// Original group ID.
        group_id: BankHircField<u32>,
        /// Original type byte when available.
        group_type_code: Option<BankHircField<u8>>,
    },
    /// One physical decision node or its readable prefix.
    Node {
        /// Physical node position.
        ordinal: u32,
        /// Original physical words.
        value: BankDialogueNode,
    },
}

/// Original words of a physical dialogue node with a readable key.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankDialogueNode {
    /// Original condition key.
    pub key: BankHircField<u32>,
    /// Original target ID or packed child index/count word.
    pub link_word: Option<BankHircField<u32>>,
    /// Original weight when its whole word fits.
    pub weight: Option<BankHircField<u16>>,
    /// Original probability when its whole word fits.
    pub probability: Option<BankHircField<u16>>,
    /// Original admitted node bytes, including any incomplete word.
    pub range: Range<u64>,
}

/// Bounded replay of `DialogueEvent` argument and physical node rows.
pub struct BankDialogueEventRows<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    groups: &'input SwitchGroups,
    tree: Option<&'input BankDialogueTree>,
    argument: u32,
    node: u32,
}

impl<'input, R> BankDialogueEventRows<'input, R> {
    pub(super) const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        groups: &'input SwitchGroups,
        tree: Option<&'input BankDialogueTree>,
    ) -> Self {
        Self {
            input,
            stopped,
            groups,
            tree,
            argument: 0,
            node: 0,
        }
    }
}

impl<R: Read + Seek> Iterator for BankDialogueEventRows<'_, R> {
    type Item = Result<BankDialogueEventRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped {
            return None;
        }
        let row = if self.argument < self.groups.ids.count {
            let ordinal = self.argument;
            self.argument += 1;
            read_group(self.input, self.groups, ordinal).map(|value| {
                BankDialogueEventRow::Argument {
                    ordinal,
                    group_id: value.id,
                    group_type_code: value.type_code,
                }
            })
        } else {
            let tree = self.tree?;
            let ordinal = self.node;
            let relative = u64::from(ordinal) * u64::from(NODE_LEN);
            let length = tree.range.end - tree.range.start;
            if relative + size_of::<u32>() as u64 > length {
                return None;
            }
            self.node += 1;
            let mut source =
                SeekObjectFields::after_id(self.input, tree.range.start, length, relative);
            read_node(&mut source).map(|value| BankDialogueEventRow::Node { ordinal, value })
        };
        if row.is_err() {
            *self.stopped = true;
        }
        Some(row)
    }
}

fn read_node<F: ObjectFields>(source: &mut F) -> Result<BankDialogueNode, F::Error> {
    if source.remaining_bytes() >= u64::from(NODE_LEN) {
        let value = hirc_music_switch::node(source)?;
        return Ok(BankDialogueNode {
            key: value.key,
            link_word: Some(value.link),
            weight: Some(value.weight),
            probability: Some(value.probability),
            range: value.range,
        });
    }

    let range = source.offset()..source.offset() + source.remaining_bytes();
    let key = source.read_u32()?;
    let link_word = (source.remaining_bytes() >= size_of::<u32>() as u64)
        .then(|| source.read_u32())
        .transpose()?;
    let weight = (source.remaining_bytes() >= size_of::<u16>() as u64)
        .then(|| source.read_u16())
        .transpose()?;
    Ok(BankDialogueNode {
        key,
        link_word,
        weight,
        probability: None,
        range,
    })
}
