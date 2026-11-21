// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::BankHircField;
use crate::hirc_node_property::{BankNodePropertyExtent, BankNodePropertyKind, WORD_LEN};
use crate::read_at::SeekSource;

/// Exact original raw words behind one property ID.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankNodePropertyValue {
    /// A property's raw word, if the complete word was available.
    Scalar(Option<BankHircField<u32>>),
    /// The independently readable raw words of a ranged modifier.
    Ranged {
        /// Raw minimum word.
        minimum: Option<BankHircField<u32>>,
        /// Raw maximum word.
        maximum: Option<BankHircField<u32>>,
    },
}

/// One original node property occurrence in authored order.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankNodePropertyRow {
    /// Zero-based position within the authored list.
    pub ordinal: u8,
    /// Original ID and byte range, including unknown IDs.
    pub id: BankHircField<u8>,
    /// Original word observations for the ID.
    pub value: BankNodePropertyValue,
}

/// Fixed-memory cursor over checked property IDs and their original words.
pub struct BankNodePropertyRows<'input, R> {
    input: &'input mut SeekSource<R>,
    extent: &'input BankNodePropertyExtent,
    ordinals: Range<u8>,
}

impl<'input, R> BankNodePropertyRows<'input, R> {
    pub(crate) const fn new(
        input: &'input mut SeekSource<R>,
        extent: &'input BankNodePropertyExtent,
    ) -> Self {
        Self {
            input,
            extent,
            ordinals: 0..extent.checked_id_count(),
        }
    }

    /// Returns the original count and checked bank-relative ranges.
    #[must_use]
    pub const fn extent(&self) -> &BankNodePropertyExtent {
        self.extent
    }
}

impl<R: Read + Seek> Iterator for BankNodePropertyRows<'_, R> {
    type Item = Result<BankNodePropertyRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let ordinal = self.ordinals.next()?;
        let row = self.read(ordinal);
        if row.is_err() {
            self.ordinals = 0..0;
        }
        Some(row)
    }
}

impl<R: Read + Seek> BankNodePropertyRows<'_, R> {
    fn read(&mut self, ordinal: u8) -> Result<BankNodePropertyRow, BankReadError> {
        let start = self.extent.ids_range.start + u64::from(ordinal);
        let mut id = [0];
        self.input.read_io_at(start, &mut id)?;
        let index = u16::from(ordinal) * self.extent.kind.words_per_id();
        let first = self.word(index)?;
        let value = match self.extent.kind {
            BankNodePropertyKind::Properties => BankNodePropertyValue::Scalar(first),
            BankNodePropertyKind::RangedModifiers => BankNodePropertyValue::Ranged {
                minimum: first,
                maximum: self.word(index + 1)?,
            },
        };
        Ok(BankNodePropertyRow {
            ordinal,
            id: BankHircField {
                value: u8::from_le_bytes(id),
                range: start..start + 1,
            },
            value,
        })
    }

    fn word(&mut self, index: u16) -> Result<Option<BankHircField<u32>>, BankReadError> {
        let Some(range) = &self.extent.words_range else {
            return Ok(None);
        };
        if index >= self.extent.checked_word_count() {
            return Ok(None);
        }
        let start = range.start + u64::from(index) * WORD_LEN;
        let mut bytes = [0; size_of::<u32>()];
        self.input.read_io_at(start, &mut bytes)?;
        Ok(Some(BankHircField {
            value: u32::from_le_bytes(bytes),
            range: start..start + WORD_LEN,
        }))
    }
}
