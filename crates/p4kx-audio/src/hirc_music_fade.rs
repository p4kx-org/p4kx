// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{BankHircField, ObjectFields};

/// Original words of one version-154 music fade.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicFadeFields {
    /// Original signed time word.
    pub time_bits: Option<BankHircField<u32>>,
    /// Original curve code, including unknown values.
    pub curve: Option<BankHircField<u32>>,
    /// Original signed offset word.
    pub offset_bits: Option<BankHircField<u32>>,
}

/// The signed word of a fade that changes timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicFadeTimingField {
    /// Original fade time.
    Time,
    /// Original fade offset.
    Offset,
}

impl BankMusicFadeFields {
    pub(crate) fn timing_evidence(&self) -> Option<(MusicFadeTimingField, &BankHircField<u32>)> {
        [
            (MusicFadeTimingField::Time, self.time_bits.as_ref()),
            (MusicFadeTimingField::Offset, self.offset_bits.as_ref()),
        ]
        .into_iter()
        .find_map(|(field, word)| {
            word.filter(|word| word.value != 0)
                .map(|word| (field, word))
        })
    }
}

pub(crate) fn read<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicFadeFields,
) -> Result<(), F::Error> {
    observed.time_bits = Some(input.read_u32()?);
    observed.curve = Some(input.read_u32()?);
    observed.offset_bits = Some(input.read_u32()?);
    Ok(())
}
