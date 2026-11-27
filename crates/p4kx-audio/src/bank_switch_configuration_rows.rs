// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_switch_container::{BankSwitchExtent, CONFIGURATION_LEN, WORD_LEN};
use crate::read_at::SeekSource;

/// One physical configuration record of a version-154 Switch container.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSwitchConfiguration {
    /// Zero-based ordinal in the authored configuration table.
    pub ordinal: u32,
    /// Original child reference ID.
    pub child_id: BankHircField<u32>,
    /// Original flags when readable, including unknown bits.
    pub flags: Option<BankHircField<u8>>,
    /// Original signed fade-out bits when the whole word fits.
    pub fade_out_bits: Option<BankHircField<u32>>,
    /// Original signed fade-in bits when the whole word fits.
    pub fade_in_bits: Option<BankHircField<u32>>,
    /// Checked original row bytes, including any incomplete final field.
    pub checked_range: Range<u64>,
}

/// Fixed-memory cursor over the original Switch child configurations.
pub struct BankSwitchConfigurationRows<'input, R> {
    input: SeekObjectFields<'input, R>,
    ordinals: Range<u32>,
    stopped: &'input mut bool,
}

impl<'input, R> BankSwitchConfigurationRows<'input, R> {
    pub(crate) const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        extent: &BankSwitchExtent,
    ) -> Self {
        let range = &extent.checked_range;
        Self {
            input: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            ordinals: 0..extent.declared_count.value,
            stopped,
        }
    }
}

impl<R: Read + Seek> Iterator for BankSwitchConfigurationRows<'_, R> {
    type Item = Result<BankSwitchConfiguration, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.input.remaining_bytes() < WORD_LEN {
            return None;
        }
        let ordinal = self.ordinals.next()?;
        let read = self.configuration(ordinal);
        if read.is_err() {
            *self.stopped = true;
        }
        Some(read)
    }
}

impl<R: Read + Seek> BankSwitchConfigurationRows<'_, R> {
    fn configuration(&mut self, ordinal: u32) -> Result<BankSwitchConfiguration, BankReadError> {
        let start = self.input.offset();
        let len = self.input.remaining_bytes().min(CONFIGURATION_LEN);
        let child_id = self.input.read_u32()?;
        let flags = (self.input.remaining_bytes() > 0)
            .then(|| self.input.read_u8())
            .transpose()?;
        let fade_out_bits = self.word()?;
        let fade_in_bits = self.word()?;
        Ok(BankSwitchConfiguration {
            ordinal,
            child_id,
            flags,
            fade_out_bits,
            fade_in_bits,
            checked_range: start..start + len,
        })
    }

    fn word(&mut self) -> Result<Option<BankHircField<u32>>, BankReadError> {
        (self.input.remaining_bytes() >= WORD_LEN)
            .then(|| self.input.read_u32())
            .transpose()
    }
}
