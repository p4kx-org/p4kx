// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{BankHircSound, BankSoundRead, MediaStorage};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_sound_origins::SoundSites;
use crate::audio_bank_sound_payload::SoundPayload;
use crate::bank_candidate_index::Resolution;
use crate::{
    CandidateInventory, MediaError, MediaWriter, ReferenceUnavailable, Status, Subresource,
};

#[path = "audio_bank_sound_loose_origins.rs"]
mod origins;

pub(crate) const RESOLUTION_NAMESPACE: &str = "audio-bank-hirc-sound-loose-wem-resolution";
pub(crate) const CANDIDATE_NAMESPACE: &str = "audio-bank-hirc-sound-loose-wem-candidate";

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LooseWemResolution {
    media_id: u32,
    resolution: Resolution,
    observed_candidates: DecimalU64,
    candidates_complete: bool,
    unavailable_reason: Option<ReferenceUnavailable>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LooseWemLink {
    target_entry_index: DecimalU64,
}

pub(super) fn project(
    projection: &BankProjection<'_>,
    version: VersionSite,
    sound: &BankHircSound,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let BankSoundRead::Decoded(source) = sound.status else {
        return Ok(());
    };
    if !source.has_media_file()
        || !matches!(
            source.storage,
            MediaStorage::Streamed | MediaStorage::PrefetchStreamed
        )
    {
        return Ok(());
    }
    writer.start_reference()?;
    let matches = projection
        .directory
        .find_loose_wems(source.source_id, writer)?;
    let unavailable_reason = match matches.inventory {
        CandidateInventory::Complete => None,
        CandidateInventory::Unavailable(reason) => Some(reason),
    };
    let complete = unavailable_reason.is_none();
    let count = matches.count() as u64;
    let indices = [sound.object.section_ordinal(), sound.object.ordinal()];
    let sites = SoundSites::new(sound, version);
    let evidence = origins::LooseReferenceOrigins::new(projection.input, &sites, writer)?;
    evidence.summary(complete, writer)?.write(
        writer,
        &Subresource::new(RESOLUTION_NAMESPACE, indices.to_vec()),
        Status::Complete,
        &SoundPayload::LooseResolution(LooseWemResolution {
            media_id: source.source_id,
            resolution: Resolution::from_inventory(complete, count),
            observed_candidates: DecimalU64::new(count),
            candidates_complete: complete,
            unavailable_reason,
        }),
        &[],
    )?;
    for (ordinal, target) in matches.entries().enumerate() {
        writer.checkpoint()?;
        let target = target?;
        let entry = writer.origins.input(matches.archive, target)?;
        let [section, sound] = indices;
        evidence.candidate(&entry, writer)?.write(
            writer,
            &Subresource::new(CANDIDATE_NAMESPACE, vec![section, sound, ordinal as u64]),
            Status::Complete,
            &SoundPayload::LooseCandidate(LooseWemLink {
                target_entry_index: DecimalU64::new(target.index() as u64),
            }),
            &[],
        )?;
    }
    Ok(())
}
