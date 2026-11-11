// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankEventAction, BankHircObject};
use p4kx_extraction::{DecimalU64, HircKind};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_projection::BankProjection;
use crate::hirc_object_index::HircObjectIndex;
use crate::{MediaError, MediaWriter, Status, Subresource};

use super::origins::EventSites;
use super::payload::EventPayload;

#[path = "audio_bank_hirc_event_reference_origins.rs"]
mod origins;

pub(crate) const RESOLUTION_NAMESPACE: &str = "audio-bank-hirc-event-action-resolution";
pub(crate) const CANDIDATE_NAMESPACE: &str = "audio-bank-hirc-event-action-candidate";

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolutionRecord {
    resolution: Resolution,
    observed_candidates: DecimalU64,
    matching_candidates: DecimalU64,
    candidates_complete: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Resolution {
    Resolved,
    Missing,
    Ambiguous,
    Unavailable,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateRecord {
    target_section_ordinal: DecimalU64,
    target_object_ordinal: DecimalU64,
    target_type_code: u8,
    action_candidate: bool,
}

pub(super) fn project(
    projection: &BankProjection<'_>,
    sites: &EventSites,
    action: &BankEventAction,
    objects: &HircObjectIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.start_reference()?;
    let candidates = objects.candidates(action.action_id.value);
    let matching = candidates
        .iter()
        .filter(|target| is_action(target, sites.version()))
        .count() as u64;
    let complete = objects.is_complete();
    let resolution = match (complete, matching) {
        (false, _) => Resolution::Unavailable,
        (true, 0) => Resolution::Missing,
        (true, 1) => Resolution::Resolved,
        (true, _) => Resolution::Ambiguous,
    };
    let indices = sites.action_indices(action.ordinal);
    let origins = origins::resolution(projection.input, sites, action, complete, writer)?;
    origins.write(
        writer,
        &Subresource::new(RESOLUTION_NAMESPACE, indices.to_vec()),
        Status::Complete,
        &EventPayload::Resolution(ResolutionRecord {
            resolution,
            observed_candidates: DecimalU64::new(candidates.len() as u64),
            matching_candidates: DecimalU64::new(matching),
            candidates_complete: complete,
        }),
        &[],
    )?;
    for (ordinal, target) in candidates.iter().enumerate() {
        let mut candidate_indices = indices.to_vec();
        candidate_indices.push(ordinal as u64);
        let origins = origins::candidate(projection.input, sites, action, target, writer)?;
        origins.write(
            writer,
            &Subresource::new(CANDIDATE_NAMESPACE, candidate_indices),
            Status::Complete,
            &EventPayload::Candidate(CandidateRecord {
                target_section_ordinal: DecimalU64::new(target.section_ordinal()),
                target_object_ordinal: DecimalU64::new(target.ordinal()),
                target_type_code: target.type_code(),
                action_candidate: is_action(target, sites.version()),
            }),
            &[],
        )?;
    }
    Ok(())
}

const fn is_action(target: &BankHircObject, version: u32) -> bool {
    matches!(target.kind(version), HircKind::Action)
}
