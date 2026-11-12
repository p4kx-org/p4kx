// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_fade::{self, BankMusicFadeFields};

/// Original version-154 `MusicTrack` Switch transition fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTrackTransitionFields {
    /// Original source fade fields.
    pub source: BankMusicFadeFields,
    /// Original synchronization code.
    pub sync_type: Option<BankHircField<u32>>,
    /// Original cue filter hash.
    pub cue_filter_hash: Option<BankHircField<u32>>,
    /// Original destination fade fields.
    pub destination: BankMusicFadeFields,
}

pub(crate) fn read<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTrackTransitionFields,
) -> Result<(), F::Error> {
    hirc_music_fade::read(input, &mut observed.source)?;
    observed.sync_type = Some(input.read_u32()?);
    observed.cue_filter_hash = Some(input.read_u32()?);
    hirc_music_fade::read(input, &mut observed.destination)
}
