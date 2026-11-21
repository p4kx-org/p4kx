// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::BankHircField;
use crate::hirc_dialogue_event::BankDialogueTree;
use crate::hirc_music_switch;

/// Physical coverage of one possible child window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankDialogueChildCoverage {
    /// The nonempty window fits the declared table and the complete physical prefix.
    Complete,
    /// The window fits the declared table, but the complete physical prefix ends before it.
    Truncated,
    /// The window extends beyond the declared whole nodes.
    OutsideDeclared,
    /// The link requests no children at a position within the declared table.
    Empty,
}

/// The child-window interpretation of a readable dialogue link word.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankDialogueChildCandidate {
    /// Possible first child ordinal from the low half-word.
    pub first: BankHircField<u16>,
    /// Possible child count from the high half-word.
    pub count: BankHircField<u16>,
    /// Complete physical nodes available at the start of the window.
    pub admitted_count: u16,
    /// The candidate's extent relative to the declared table and the complete physical prefix.
    pub coverage: BankDialogueChildCoverage,
}

impl BankDialogueChildCandidate {
    pub(crate) fn from_link(link: &BankHircField<u32>, tree: &BankDialogueTree) -> Self {
        let [first, count] = hirc_music_switch::child_link_words(link.value);
        let middle = link.range.start + size_of::<u16>() as u64;
        let available = tree.complete_nodes().saturating_sub(u32::from(first));
        let admitted_count = count.min(u16::try_from(available).unwrap_or(u16::MAX));
        let end = u32::from(first) + u32::from(count);
        let coverage = if end > tree.declared_nodes() {
            BankDialogueChildCoverage::OutsideDeclared
        } else if count == 0 {
            BankDialogueChildCoverage::Empty
        } else if admitted_count < count {
            BankDialogueChildCoverage::Truncated
        } else {
            BankDialogueChildCoverage::Complete
        };
        Self {
            first: BankHircField {
                value: first,
                range: link.range.start..middle,
            },
            count: BankHircField {
                value: count,
                range: middle..link.range.end,
            },
            admitted_count,
            coverage,
        }
    }
}
