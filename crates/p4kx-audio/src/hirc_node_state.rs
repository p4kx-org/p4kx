// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::SectionId;
use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_state;

/// Original property mapping fields of a v154 inline State chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankStateBinding {
    /// Authored variable-length RTPC property ID.
    pub property_id: BankHircField<u32>,
    /// Unmodified accumulation type byte.
    pub accumulation: BankHircField<u8>,
    /// Unmodified in-dB byte.
    pub in_db: BankHircField<u8>,
}

/// Original header of a v154 inline State group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankStateGroup {
    /// Original group ID.
    pub group_id: BankHircField<u32>,
    /// Unmodified synchronization type byte.
    pub synchronization: BankHircField<u8>,
    /// Authored variable-length State count.
    pub state_count: BankHircField<u32>,
}

pub(crate) fn read_count<F: ObjectFields>(fields: &mut F) -> Result<BankHircField<u32>, F::Error> {
    fields.read_variable_length(SectionId::HIRC)
}

pub(crate) fn read_binding<F: ObjectFields>(fields: &mut F) -> Result<BankStateBinding, F::Error> {
    Ok(BankStateBinding {
        property_id: fields.read_variable_length(SectionId::HIRC)?,
        accumulation: fields.read_u8()?,
        in_db: fields.read_u8()?,
    })
}

pub(crate) fn read_group<F: ObjectFields>(fields: &mut F) -> Result<BankStateGroup, F::Error> {
    Ok(BankStateGroup {
        group_id: fields.read_u32()?,
        synchronization: fields.read_u8()?,
        state_count: read_count(fields)?,
    })
}

pub(crate) fn skip<F: ObjectFields>(fields: &mut F) -> Result<Range<u64>, F::Error> {
    let start = fields.offset();
    for _ in 0..read_count(fields)?.value {
        read_binding(fields)?;
    }
    for _ in 0..read_count(fields)?.value {
        let group = read_group(fields)?;
        for _ in 0..group.state_count.value {
            fields.read_u32()?;
            hirc_state::skip_properties(fields)?;
        }
    }
    Ok(start..fields.offset())
}
