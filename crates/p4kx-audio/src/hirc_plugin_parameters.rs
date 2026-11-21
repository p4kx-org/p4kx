// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

const BANK_REFERENCE_LEN: u64 = 1 + size_of::<u32>() as u64;

/// The declared size and admitted original bytes of an opaque plug-in payload.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankPluginParameters {
    /// Original size word.
    pub size: BankHircField<u32>,
    /// Original payload bytes, including a checked truncated prefix.
    pub range: Range<u64>,
}

impl BankPluginParameters {
    /// Returns whether the full declared payload fits the physical object.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        u64::from(self.size.value) == self.range.end - self.range.start
    }
}

/// Original adapter metadata shared by effects and devices.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankPluginFields {
    /// Original plug-in ID and encoding bits.
    pub plugin_id: Option<BankHircField<u32>>,
    /// Opaque payload observed once the original size word fits.
    pub parameters: Option<BankPluginParameters>,
    /// Authored one-byte number of bank-data references, including zero.
    pub bank_data_count: Option<BankHircField<u8>>,
    /// Bank-relative range of the complete checked references, in source order.
    pub bank_data_range: Option<Range<u64>>,
}

impl BankPluginFields {
    pub(crate) fn checked_end(&self) -> Option<u64> {
        self.bank_data_range
            .as_ref()
            .map(|range| range.end)
            .or_else(|| self.parameters.as_ref().map(|value| value.range.end))
            .or_else(|| self.plugin_id.as_ref().map(|field| field.range.end))
    }
}

/// A physical reference from a plug-in to source bank data.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankPluginDataReference {
    /// Original media mapping index, including explicit zero and duplicates.
    pub index: BankHircField<u8>,
    /// Original referenced source ID.
    pub source_id: BankHircField<u32>,
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankPluginFields,
) -> Result<(), F::Error> {
    fields.plugin_id = Some(input.read_u32()?);
    let size = input.read_u32()?;
    let start = input.offset();
    let length = u64::from(size.value);
    let admitted = length.min(input.remaining_bytes());
    fields.parameters = Some(BankPluginParameters {
        size,
        range: start..start + admitted,
    });
    input.skip_bytes(length)?;
    let count = input.read_u8()?;
    let length = u64::from(count.value) * BANK_REFERENCE_LEN;
    fields.bank_data_count = Some(count);
    let start = input.offset();
    let complete = input.remaining_bytes() / BANK_REFERENCE_LEN * BANK_REFERENCE_LEN;
    fields.bank_data_range = Some(start..start + length.min(complete));
    input.skip_bytes(length)
}

pub(crate) fn read_reference<F: ObjectFields>(
    input: &mut F,
) -> Result<BankPluginDataReference, F::Error> {
    Ok(BankPluginDataReference {
        index: input.read_u8()?,
        source_id: input.read_u32()?,
    })
}
