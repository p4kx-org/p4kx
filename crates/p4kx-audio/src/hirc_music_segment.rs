// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::ObjectFields;
use crate::hirc_music_node::BankMusicNodeFields;
use crate::{BankError, BankHircField, SectionId};

/// Original scalar fields of a segment, including the exact numeric representations.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicSegmentFields {
    /// Original shared music node fields.
    pub node: BankMusicNodeFields,
    /// Duration stored as the exact binary64 word.
    pub duration: Option<BankHircField<u64>>,
    /// Original marker count.
    pub marker_count: Option<BankHircField<u32>>,
}

pub(crate) fn header<F: ObjectFields>(
    reader: &mut F,
    fields: &mut BankMusicSegmentFields,
) -> Result<(f64, u32), F::Error> {
    let duration = reader.scalar(u64::from_le_bytes)?;
    let milliseconds = f64::from_bits(duration.value);
    fields.duration = Some(duration);
    let count = reader.read_u32()?;
    let markers = count.value;
    fields.marker_count = Some(count);
    Ok((milliseconds, markers))
}

pub(crate) struct MarkerWords {
    pub(crate) id: BankHircField<u32>,
    pub(crate) position: BankHircField<u64>,
}

pub(crate) fn marker<F: ObjectFields>(reader: &mut F) -> Result<MarkerWords, F::Error> {
    Ok(MarkerWords {
        id: reader.read_u32()?,
        position: reader.scalar(u64::from_le_bytes)?,
    })
}

pub(crate) fn finish<F: ObjectFields>(reader: &F) -> Result<(), F::Error> {
    if reader.remaining_bytes() > 0 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "bytes follow the fields of a music object",
        )
        .into());
    }
    Ok(())
}
