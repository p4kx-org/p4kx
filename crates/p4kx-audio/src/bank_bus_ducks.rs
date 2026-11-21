// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::read_at::SeekSource;

use crate::hirc_bus_fields::DUCK_LEN;

/// Original authored duck-target fields.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankBusDuckRow {
    /// Zero-based occurrence in the source list.
    pub ordinal: u32,
    /// Original target Bus ID, including zero.
    pub bus_id: BankHircField<u32>,
    /// Raw binary32 duck-volume word.
    pub volume_bits: BankHircField<u32>,
    /// Original signed fade-out time.
    pub fade_out_time: BankHircField<i32>,
    /// Original signed fade-in time.
    pub fade_in_time: BankHircField<i32>,
    /// Original fade curve code.
    pub fade_curve: BankHircField<u8>,
    /// Original target property code.
    pub target_property: BankHircField<u8>,
}

/// Fixed-memory cursor over complete checked duck-target rows.
pub struct BankBusDuckRows<'input, R> {
    input: &'input mut SeekSource<R>,
    range: Range<u64>,
    ordinal: u32,
}

impl<'input, R> BankBusDuckRows<'input, R> {
    pub(super) const fn new(input: &'input mut SeekSource<R>, range: Range<u64>) -> Self {
        Self {
            input,
            range,
            ordinal: 0,
        }
    }
}

impl<R: Read + Seek> Iterator for BankBusDuckRows<'_, R> {
    type Item = Result<BankBusDuckRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.range.is_empty() {
            return None;
        }
        let result = self.read();
        if result.is_err() {
            self.range.start = self.range.end;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankBusDuckRows<'_, R> {
    fn read(&mut self) -> Result<BankBusDuckRow, BankReadError> {
        let mut row = SeekObjectFields::after_id(self.input, self.range.start, DUCK_LEN, 0);
        let result = BankBusDuckRow {
            ordinal: self.ordinal,
            bus_id: row.read_u32()?,
            volume_bits: row.read_u32()?,
            fade_out_time: row.scalar(i32::from_le_bytes)?,
            fade_in_time: row.scalar(i32::from_le_bytes)?,
            fade_curve: row.read_u8()?,
            target_property: row.read_u8()?,
        };
        self.ordinal += 1;
        self.range.start += DUCK_LEN;
        Ok(result)
    }
}
