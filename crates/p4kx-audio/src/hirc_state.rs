// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::{BankError, SectionId};

const FIRST_STANDALONE_VERSION: u32 = 128;
const LAST_STANDALONE_VERSION: u32 = 145;
const ID_BYTES: u64 = size_of::<u16>() as u64;
const VALUE_BYTES: u64 = size_of::<u32>() as u64;

/// A standalone State's property bundle and its unconsumed body suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StateObject {
    /// Ordered original State properties.
    pub properties: StateProperties,
    /// The remaining half-open bank-relative range after the bundle.
    pub trailing_range: Range<u64>,
}

/// The original planar State property bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateProperties {
    count: BankHircField<u16>,
    words: Vec<PropertyWords>,
    values_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PropertyWords {
    id: u16,
    bits: u32,
}

/// One State property with its original ID and binary32 word.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StateProperty {
    /// The authored RTPC parameter ID.
    pub id: BankHircField<u16>,
    /// The unmodified IEEE-754 binary32 word.
    pub value_bits: BankHircField<u32>,
}

impl StateProperty {
    /// Returns the original word interpreted as IEEE-754 binary32.
    #[must_use]
    pub const fn value(&self) -> f32 {
        f32::from_bits(self.value_bits.value)
    }
}

impl StateProperties {
    /// Returns the original property count and its range.
    #[must_use]
    pub const fn count(&self) -> &BankHircField<u16> {
        &self.count
    }

    /// Returns the half-open bank-relative range of the complete property bundle.
    #[must_use]
    pub fn consumed_range(&self) -> Range<u64> {
        self.count.range.start..self.values_offset + u64::from(self.count.value) * VALUE_BYTES
    }

    /// Returns every property in source order, including repeated or unknown IDs.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = StateProperty> + '_ {
        self.words.iter().enumerate().map(|(position, word)| {
            let index = position as u64;
            let id = self.count.range.end + index * ID_BYTES;
            let value = self.values_offset + index * VALUE_BYTES;
            StateProperty {
                id: BankHircField {
                    value: word.id,
                    range: id..id + ID_BYTES,
                },
                value_bits: BankHircField {
                    value: word.bits,
                    range: value..value + VALUE_BYTES,
                },
            }
        })
    }
}

pub(crate) const fn supports_standalone(version: u32) -> bool {
    version >= FIRST_STANDALONE_VERSION && version <= LAST_STANDALONE_VERSION
}

pub(crate) fn read_object<F: ObjectFields>(
    fields: &mut F,
    bank_offset: u64,
) -> Result<StateObject, F::Error> {
    let properties = read_properties(fields, bank_offset)?;
    let tail = bank_offset + fields.offset();
    Ok(StateObject {
        properties,
        trailing_range: tail..tail + fields.remaining_bytes(),
    })
}

pub(crate) fn read_properties<F: ObjectFields>(
    fields: &mut F,
    bank_offset: u64,
) -> Result<StateProperties, F::Error> {
    let mut count = fields.read_u16()?;
    count.range.start += bank_offset;
    count.range.end += bank_offset;
    let needed = u64::from(count.value) * (ID_BYTES + VALUE_BYTES);
    let available = fields.remaining_bytes();
    if needed > available {
        return Err(BankError::truncated(
            SectionId::HIRC,
            usize::try_from(available).unwrap_or(usize::MAX),
            needed,
        )
        .into());
    }
    let mut words = Vec::with_capacity(usize::from(count.value));
    for _ in 0..count.value {
        words.push(PropertyWords {
            id: fields.read_u16()?.value,
            bits: 0,
        });
    }
    let values_offset = bank_offset + fields.offset();
    for word in &mut words {
        word.bits = fields.read_u32()?.value;
    }
    Ok(StateProperties {
        count,
        words,
        values_offset,
    })
}
