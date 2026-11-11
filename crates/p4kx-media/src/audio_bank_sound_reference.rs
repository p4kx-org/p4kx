// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{BankHircSound, BankSoundRead, MediaStorage};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_didx_index::DidxIndex;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_sound_payload::SoundPayload;
use crate::bank_candidate_index::Resolution;
use crate::{MediaError, MediaWriter, Status, Subresource};

#[path = "audio_bank_sound_reference_origins.rs"]
mod origins;

pub(crate) const RESOLUTION_NAMESPACE: &str = "audio-bank-hirc-sound-media-resolution";
pub(crate) const CANDIDATE_NAMESPACE: &str = "audio-bank-hirc-sound-media-candidate";

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SoundMediaResolution {
    resolution: Resolution,
    observed_candidates: DecimalU64,
    candidates_complete: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SoundMediaCandidate {
    target_section_ordinal: DecimalU64,
    target_triplet_ordinal: DecimalU64,
}

pub(crate) fn project(
    projection: &BankProjection<'_>,
    version: VersionSite,
    sound: &BankHircSound,
    didx: &DidxIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let BankSoundRead::Decoded(source) = sound.status else {
        return Ok(());
    };
    if !source.has_media_file()
        || !matches!(
            source.storage,
            MediaStorage::Embedded | MediaStorage::PrefetchStreamed
        )
    {
        return Ok(());
    }
    writer.start_reference()?;
    let indices = [sound.object.section_ordinal(), sound.object.ordinal()];
    let candidates = didx.candidates(source.source_id);
    let count = candidates.len() as u64;
    let complete = didx.is_complete();
    let sites = origins::SoundReferenceSites::new(sound, version);
    sites.summary(projection.input, complete, writer)?.write(
        writer,
        &Subresource::new(RESOLUTION_NAMESPACE, indices.to_vec()),
        Status::Complete,
        &SoundPayload::Resolution(SoundMediaResolution {
            resolution: didx.resolution(count),
            observed_candidates: DecimalU64::new(count),
            candidates_complete: complete,
        }),
        &[],
    )?;
    for (ordinal, target) in candidates.iter().enumerate() {
        let mut position = indices.to_vec();
        position.push(ordinal as u64);
        sites.candidate(projection.input, target, writer)?.write(
            writer,
            &Subresource::new(CANDIDATE_NAMESPACE, position),
            Status::Complete,
            &SoundPayload::Candidate(SoundMediaCandidate {
                target_section_ordinal: DecimalU64::new(target.section),
                target_triplet_ordinal: DecimalU64::new(target.triplet),
            }),
            &[],
        )?;
    }
    Ok(())
}
