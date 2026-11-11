// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::{BankError, SectionId};

const LAST_FIXED_EVENT_COUNT_VERSION: u32 = 122;

/// Original representation of an Event action count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankEventCountEncoding {
    /// Little-endian unsigned 32-bit word.
    FixedU32,
    /// Wwise variable-length unsigned integer.
    VariableLength,
}

impl BankEventCountEncoding {
    pub(crate) const fn for_version(version: u32) -> Self {
        if version <= LAST_FIXED_EVENT_COUNT_VERSION {
            Self::FixedU32
        } else {
            Self::VariableLength
        }
    }
}

pub(crate) fn count<F: ObjectFields>(
    encoding: BankEventCountEncoding,
    fields: &mut F,
) -> Result<BankHircField<u32>, F::Error> {
    match encoding {
        BankEventCountEncoding::FixedU32 => fields.read_u32(),
        BankEventCountEncoding::VariableLength => fields.read_variable_length(SectionId::HIRC),
    }
}

pub(crate) const fn require_actions(count: u32, remaining: u64) -> Result<(), BankError> {
    if count as u64 > remaining / OBJECT_ID_LEN as u64 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "an event lists more actions than its body holds",
        ));
    }
    Ok(())
}

pub(crate) fn read(version: u32, fields: &mut SectionReader<'_>) -> Result<Vec<u32>, BankError> {
    let count = count(BankEventCountEncoding::for_version(version), fields)?.value;
    require_actions(count, fields.remaining_bytes())?;
    let mut actions = Vec::with_capacity(usize::try_from(count).unwrap_or(usize::MAX));
    for _ in 0..count {
        actions.push(fields.read_u32()?.value);
    }
    Ok(actions)
}
