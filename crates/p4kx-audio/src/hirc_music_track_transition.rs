// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{BankHircField, ObjectFields};

/// Original version-154 `MusicTrack` Switch transition fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTrackTransitionFields {
    /// Original source fade fields.
    pub source: BankMusicTrackFadeFields,
    /// Original synchronization code.
    pub sync_type: Option<BankHircField<u32>>,
    /// Original cue filter hash.
    pub cue_filter_hash: Option<BankHircField<u32>>,
    /// Original destination fade fields.
    pub destination: BankMusicTrackFadeFields,
}

/// Original raw words of one `MusicTrack` Switch fade.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTrackFadeFields {
    /// Original signed time word.
    pub time_bits: Option<BankHircField<u32>>,
    /// Original curve code, including unknown values.
    pub curve: Option<BankHircField<u32>>,
    /// Original signed offset word.
    pub offset_bits: Option<BankHircField<u32>>,
}

pub(crate) fn read<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTrackTransitionFields,
) -> Result<(), F::Error> {
    fade(input, &mut observed.source)?;
    observed.sync_type = Some(input.read_u32()?);
    observed.cue_filter_hash = Some(input.read_u32()?);
    fade(input, &mut observed.destination)
}

fn fade<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTrackFadeFields,
) -> Result<(), F::Error> {
    observed.time_bits = Some(input.read_u32()?);
    observed.curve = Some(input.read_u32()?);
    observed.offset_bits = Some(input.read_u32()?);
    Ok(())
}
