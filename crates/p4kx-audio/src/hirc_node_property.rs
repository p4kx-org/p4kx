// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

pub(crate) const WORD_LEN: u64 = size_of::<u32>() as u64;

/// The authored property list within a version-154 node base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankNodePropertyKind {
    /// One raw value word for each property ID.
    Properties,
    /// A raw minimum and maximum word for each property ID.
    RangedModifiers,
}

impl BankNodePropertyKind {
    pub(crate) const fn words_per_id(self) -> u16 {
        match self {
            Self::Properties => 1,
            Self::RangedModifiers => 2,
        }
    }
}

/// The original count and checked prefixes of a planar property list.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankNodePropertyExtent {
    /// Original one-byte list count.
    pub declared_count: BankHircField<u8>,
    /// Bank-relative prefix of readable property IDs.
    pub ids_range: Range<u64>,
    /// Prefix of complete value words once the full ID array fits.
    pub words_range: Option<Range<u64>>,
    pub(crate) kind: BankNodePropertyKind,
    checked_ids: u8,
    checked_words: u16,
}

impl BankNodePropertyExtent {
    /// Returns the number of checked ID bytes.
    #[must_use]
    pub const fn checked_id_count(&self) -> u8 {
        self.checked_ids
    }

    /// Returns the number of complete checked words.
    #[must_use]
    pub const fn checked_word_count(&self) -> u16 {
        self.checked_words
    }

    /// Returns whether every authored ID and value word fits.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.checked_ids == self.declared_count.value
            && self.checked_words == u16::from(self.declared_count.value) * self.kind.words_per_id()
    }
}

/// Separately observed original property and ranged modifier lists.
#[derive(Debug, Default)]
pub struct BankNodePropertyExtents {
    properties: Option<BankNodePropertyExtent>,
    ranged_modifiers: Option<BankNodePropertyExtent>,
}

impl BankNodePropertyExtents {
    /// Returns the checked list when its original count was read.
    #[must_use]
    pub const fn get(&self, kind: BankNodePropertyKind) -> Option<&BankNodePropertyExtent> {
        match kind {
            BankNodePropertyKind::Properties => self.properties.as_ref(),
            BankNodePropertyKind::RangedModifiers => self.ranged_modifiers.as_ref(),
        }
    }
}

pub(crate) fn scan<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankNodePropertyExtents,
) -> Result<(), F::Error> {
    scan_list(
        fields,
        BankNodePropertyKind::Properties,
        &mut observed.properties,
    )?;
    scan_list(
        fields,
        BankNodePropertyKind::RangedModifiers,
        &mut observed.ranged_modifiers,
    )
}

fn scan_list<F: ObjectFields>(
    fields: &mut F,
    kind: BankNodePropertyKind,
    observed: &mut Option<BankNodePropertyExtent>,
) -> Result<(), F::Error> {
    let count = fields.read_u8()?;
    let start = fields.offset();
    let available = fields.remaining_bytes();
    let checked_ids = count.value.min(u8::try_from(available).unwrap_or(u8::MAX));
    let ids_range = start..start + u64::from(checked_ids);
    let declared_words = u16::from(count.value) * kind.words_per_id();
    let (words_range, checked_words) = if checked_ids == count.value {
        let available_words = (available - u64::from(checked_ids)) / WORD_LEN;
        let checked = declared_words.min(u16::try_from(available_words).unwrap_or(u16::MAX));
        (
            Some(ids_range.end..ids_range.end + u64::from(checked) * WORD_LEN),
            checked,
        )
    } else {
        (None, 0)
    };
    let list_bytes = u64::from(count.value) + u64::from(declared_words) * WORD_LEN;
    *observed = Some(BankNodePropertyExtent {
        declared_count: count,
        ids_range,
        words_range,
        kind,
        checked_ids,
        checked_words,
    });
    fields.skip_bytes(list_bytes)
}
