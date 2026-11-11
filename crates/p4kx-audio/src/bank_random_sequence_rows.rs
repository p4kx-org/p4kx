// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::BankHircField;
use crate::hirc_random_sequence::{ITEM_LEN, WORD_LEN};
use crate::read_at::SeekSource;

/// One physical playlist item from a version-154 Random/Sequence container.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankRandomSequenceItem {
    /// Zero-based ordinal in the original playlist.
    pub ordinal: u16,
    /// Original child reference ID.
    pub play_id: BankHircField<u32>,
    /// Original signed weight bits when the whole word fits.
    pub weight_bits: Option<BankHircField<u32>>,
    /// Checked original row bytes, including an incomplete final weight.
    pub checked_range: Range<u64>,
}

impl BankRandomSequenceItem {
    /// Returns the original signed 32-bit weight when readable.
    #[must_use]
    pub fn signed_weight(&self) -> Option<i32> {
        self.weight_bits
            .as_ref()
            .map(|word| i32::from_le_bytes(word.value.to_le_bytes()))
    }
}

/// Sequential cursor over checked playlist rows in their owning bank.
pub struct BankRandomSequenceRows<'input, R> {
    input: &'input mut SeekSource<R>,
    remaining: Range<u64>,
    ordinal: u16,
}

impl<'input, R> BankRandomSequenceRows<'input, R> {
    pub(crate) const fn new(input: &'input mut SeekSource<R>, remaining: Range<u64>) -> Self {
        Self {
            input,
            remaining,
            ordinal: 0,
        }
    }
}

impl<R: Read + Seek> Iterator for BankRandomSequenceRows<'_, R> {
    type Item = Result<BankRandomSequenceItem, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.end - self.remaining.start < WORD_LEN as u64 {
            return None;
        }
        let result = self.read();
        if result.is_err() {
            self.remaining.start = self.remaining.end;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankRandomSequenceRows<'_, R> {
    fn read(&mut self) -> Result<BankRandomSequenceItem, BankReadError> {
        let start = self.remaining.start;
        let length = (self.remaining.end - start).min(ITEM_LEN as u64);
        let (play_id, weight_bits) = if length == ITEM_LEN as u64 {
            let mut words = [[0; WORD_LEN]; 2];
            self.input.read_io_at(start, words.as_flattened_mut())?;
            let [play_id, weight_bits] = words;
            (
                u32::from_le_bytes(play_id),
                Some(u32::from_le_bytes(weight_bits)),
            )
        } else {
            let mut bytes = [0; WORD_LEN];
            self.input.read_io_at(start, &mut bytes)?;
            (u32::from_le_bytes(bytes), None)
        };
        let weight_start = start + WORD_LEN as u64;
        let item = BankRandomSequenceItem {
            ordinal: self.ordinal,
            play_id: BankHircField {
                value: play_id,
                range: start..weight_start,
            },
            weight_bits: weight_bits.map(|value| BankHircField {
                value,
                range: weight_start..start + length,
            }),
            checked_range: start..start + length,
        };
        self.remaining.start += length;
        self.ordinal += 1;
        Ok(item)
    }
}
