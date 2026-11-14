// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_node::MusicList;
use crate::hirc_music_switch::{NODE_LEN, SwitchGroups, invalid};
use crate::hirc_node_property::{self, BankNodePropertyExtents};

/// Original fields of a physical `DialogueEvent`.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankDialogueEventFields {
    /// Original event probability byte.
    pub probability: Option<BankHircField<u8>>,
    /// Original decision depth and argument count.
    pub argument_count: Option<BankHircField<u32>>,
    /// Original tree byte length.
    pub tree_size: Option<BankHircField<u32>>,
    /// Original decision mode code.
    pub mode_code: Option<BankHircField<u8>>,
    /// Checked tree byte prefix, when the complete tree header is readable.
    pub tree: Option<BankDialogueTree>,
    /// Original planar property lists.
    pub properties: BankNodePropertyExtents,
}

/// Coverage of the declared tree range, independent of link semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankDialogueTreeCoverage {
    /// All declared tree bytes form complete physical nodes.
    Complete,
    /// The original body ends before the declared tree boundary.
    Truncated,
    /// The declared tree bytes fit but do not form whole physical nodes.
    Misaligned,
}

/// Checked extent and coverage of a `DialogueEvent` decision tree.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankDialogueTree {
    /// Original admitted byte prefix.
    pub range: Range<u64>,
    /// Coverage of the declared physical table.
    pub coverage: BankDialogueTreeCoverage,
    length: u32,
}

impl BankDialogueTree {
    /// Returns the number of complete physical node rows.
    #[must_use]
    pub const fn complete_nodes(&self) -> u32 {
        self.length / NODE_LEN
    }

    /// Returns the number of bytes in a trailing partial node.
    #[must_use]
    pub const fn partial_node_bytes(&self) -> u32 {
        self.length % NODE_LEN
    }

    /// Returns the original trailing partial node range when present.
    #[must_use]
    pub fn partial_node_range(&self) -> Option<Range<u64>> {
        let start = self.range.start + u64::from(self.complete_nodes()) * u64::from(NODE_LEN);
        (start < self.range.end).then_some(start..self.range.end)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankDialogueEventFields,
    arguments: &mut SwitchGroups,
) -> Result<(), F::Error> {
    fields.probability = Some(input.read_u8()?);
    let count = input.read_u32()?;
    let depth = count.value;
    fields.argument_count = Some(count);
    arguments.ids = checked_arguments(input, depth, size_of::<u32>());
    input.skip_bytes(u64::from(depth) * size_of::<u32>() as u64)?;
    arguments.types = checked_arguments(input, depth, size_of::<u8>());
    input.skip_bytes(u64::from(depth))?;

    let size = input.read_u32()?;
    let declared = size.value;
    fields.tree_size = Some(size);
    fields.mode_code = Some(input.read_u8()?);
    let start = input.offset();
    let admitted = u32::try_from(input.remaining_bytes())
        .unwrap_or(u32::MAX)
        .min(declared);
    let coverage = if admitted < declared {
        BankDialogueTreeCoverage::Truncated
    } else if !declared.is_multiple_of(NODE_LEN) {
        BankDialogueTreeCoverage::Misaligned
    } else {
        BankDialogueTreeCoverage::Complete
    };
    fields.tree = Some(BankDialogueTree {
        range: start..start + u64::from(admitted),
        length: admitted,
        coverage,
    });

    input.skip_bytes(u64::from(declared))?;
    hirc_node_property::scan(input, &mut fields.properties)?;
    if coverage == BankDialogueTreeCoverage::Misaligned {
        return Err(invalid("a dialogue decision tree is no whole number of nodes").into());
    }
    if input.remaining_bytes() != 0 {
        return Err(invalid("bytes follow the DialogueEvent property lists").into());
    }
    Ok(())
}

fn checked_arguments<F: ObjectFields>(input: &F, declared: u32, width: usize) -> MusicList {
    let width = width as u64;
    let available = u32::try_from(input.remaining_bytes() / width).unwrap_or(u32::MAX);
    let count = available.min(declared);
    MusicList {
        start: input.offset(),
        length: u64::from(count) * width,
        count,
    }
}
