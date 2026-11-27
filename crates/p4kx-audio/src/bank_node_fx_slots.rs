// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::BankHircField;
use crate::hirc_node_fx::SLOT_LEN;
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, SectionId};

/// Original readable fields of one version-154 effect or metadata slot.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankNodeFxSlot {
    /// Physical zero-based position in the authored list.
    pub ordinal: u8,
    /// Checked original slot bytes, including an incomplete final field.
    pub checked_range: Range<u64>,
    /// Authored slot index, including repeated indices.
    pub index: BankHircField<u8>,
    /// Original effect reference, including zero, when its full word fits.
    pub effect_id: Option<BankHircField<u32>>,
    /// Original effect flags or metadata share-set byte.
    pub bits: Option<BankHircField<u8>>,
}

/// Fixed-memory cursor over checked original effect or metadata slot prefixes.
pub struct BankNodeFxSlots<'input, R> {
    input: &'input mut SeekSource<R>,
    remaining: Range<u64>,
    ordinal: u8,
}

impl<'input, R> BankNodeFxSlots<'input, R> {
    pub(crate) const fn new(input: &'input mut SeekSource<R>, remaining: Range<u64>) -> Self {
        Self {
            input,
            remaining,
            ordinal: 0,
        }
    }
}

impl<R: Read + Seek> Iterator for BankNodeFxSlots<'_, R> {
    type Item = Result<BankNodeFxSlot, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let result = self.read();
        if result.is_err() {
            self.remaining.start = self.remaining.end;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankNodeFxSlots<'_, R> {
    fn read(&mut self) -> Result<BankNodeFxSlot, BankReadError> {
        let start = self.remaining.start;
        let len = (self.remaining.end - start).min(SLOT_LEN as u64);
        let mut bytes = [0; SLOT_LEN];
        let output = usize::try_from(len)
            .ok()
            .and_then(|len| bytes.get_mut(..len))
            .ok_or_else(|| {
                BankError::invalid(SectionId::HIRC, "effect slot exceeds its checked buffer")
            })?;
        self.input.read_io_at(start, output)?;
        let [index, id0, id1, id2, id3, bits] = bytes;
        let id_end = SLOT_LEN as u64 - 1;
        let result = BankNodeFxSlot {
            ordinal: self.ordinal,
            checked_range: start..start + len,
            index: BankHircField {
                value: index,
                range: start..start + 1,
            },
            effect_id: (len >= id_end).then_some(BankHircField {
                value: u32::from_le_bytes([id0, id1, id2, id3]),
                range: start + 1..start + id_end,
            }),
            bits: (len == SLOT_LEN as u64).then(|| BankHircField {
                value: bits,
                range: start + id_end..start + len,
            }),
        };
        self.remaining.start += len;
        self.ordinal += 1;
        Ok(result)
    }
}
