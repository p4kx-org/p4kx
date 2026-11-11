// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankHircSound, BankSoundRead};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_sound_origins;
use crate::audio_bank_sound_payload::SoundRecord;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) use crate::audio_bank_sound_payload::SoundPayload;

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-sound";

pub(crate) fn project(
    projection: &BankProjection<'_>,
    version: VersionSite,
    sound: &BankHircSound,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (record, status, issue) = match &sound.status {
        BankSoundRead::Decoded(source) => (
            SoundRecord::decoded(*source, &sound.fields)?,
            Status::Complete,
            None,
        ),
        BankSoundRead::Malformed(error) => (
            SoundRecord::malformed(&sound.fields)?,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-sound",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        ),
        BankSoundRead::Unsupported(error) => (
            SoundRecord::unsupported(),
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-sound",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        ),
        _ => {
            return Err(MediaError::Contract(
                "unexpected HIRC Sound reader state".into(),
            ));
        }
    };
    let prepared = audio_bank_sound_origins::prepare(projection.input, sound, version, writer)?;
    prepared.write(
        writer,
        &Subresource::new(
            NAMESPACE,
            vec![sound.object.section_ordinal(), sound.object.ordinal()],
        ),
        status,
        &SoundPayload::BankHircSound(record),
        issue.as_slice(),
    )
}
