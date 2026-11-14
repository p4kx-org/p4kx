// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankHircObject;
use p4kx_extraction::{ActionTarget, DecimalU64, HircKind};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_projection::BankProjection;
use crate::audio_foreign::Selector as ForeignTarget;
use crate::bank_candidate_index::Resolution;
use crate::hirc_object_index::HircObjectIndex;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, Status, Subresource};

use super::ActionPayload;
use super::origins::Sites;

#[path = "audio_bank_hirc_action_reference_origins.rs"]
mod origins;

pub(crate) const RESOLUTION_NAMESPACE: &str = "audio-bank-hirc-action-target-resolution";
pub(crate) const CANDIDATE_NAMESPACE: &str = "audio-bank-hirc-action-target-candidate";

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionTargetResolution {
    target_id: u32,
    local: LocalTargetInventory,
    foreign: ForeignLookup,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LocalTargetInventory {
    resolution: Resolution,
    observed_candidates: DecimalU64,
    matching_candidates: DecimalU64,
    candidates_complete: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum ForeignLookup {
    LocalOnly,
    LocalMatch {
        target: ForeignTarget,
    },
    Unavailable {
        target: ForeignTarget,
        reason: ForeignUnavailable,
    },
    DirectorySearch {
        target: ForeignTarget,
        query_index: DecimalU64,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ForeignUnavailable {
    IncompleteLocalInventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionTargetCandidate {
    target_section_ordinal: DecimalU64,
    target_object_ordinal: DecimalU64,
    target_type_code: u8,
    matches_target_kind: bool,
}

pub(super) fn project(
    projection: &BankProjection<'_>,
    sites: &Sites,
    target: ActionTarget,
    objects: &HircObjectIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let foreign = if target.is_play_event() {
        Some(ForeignTarget::Event)
    } else if target.is_play() {
        target
            .bank
            .filter(|bank| bank.bank_id != 0)
            .map(|bank| ForeignTarget::Bank {
                bank_id: bank.bank_id,
            })
    } else {
        return Ok(());
    };
    writer.start_reference()?;
    let candidates = objects.candidates(target.target_id);
    let matches = |candidate: &BankHircObject| {
        !matches!(foreign, Some(ForeignTarget::Event))
            || candidate.kind(sites.version()) == HircKind::Event
    };
    let mut matching = 0;
    for candidate in candidates {
        writer.checkpoint()?;
        matching += u64::from(matches(candidate));
    }
    let complete = objects.is_complete();
    let foreign = foreign_lookup(
        projection,
        sites,
        target.target_id,
        foreign,
        matching,
        complete,
        writer,
    )?;
    let indices = sites.indices();
    origins::resolution(projection.input, sites, complete, &foreign, writer)?.write(
        writer,
        &Subresource::new(RESOLUTION_NAMESPACE, indices.to_vec()),
        Status::Complete,
        &ActionPayload::TargetResolution(ActionTargetResolution {
            target_id: target.target_id,
            local: LocalTargetInventory {
                resolution: objects.resolution(matching),
                observed_candidates: DecimalU64::new(candidates.len() as u64),
                matching_candidates: DecimalU64::new(matching),
                candidates_complete: complete,
            },
            foreign,
        }),
        &[],
    )?;
    write_candidates(projection.input, sites, candidates, matches, writer)
}

fn write_candidates(
    input: &MediaOriginInput,
    sites: &Sites,
    candidates: &[BankHircObject],
    matches: impl Fn(&BankHircObject) -> bool,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let [section, action] = sites.indices();
    for (ordinal, candidate) in candidates.iter().enumerate() {
        writer.checkpoint()?;
        origins::candidate(input, sites, candidate, writer)?.write(
            writer,
            &Subresource::new(CANDIDATE_NAMESPACE, vec![section, action, ordinal as u64]),
            Status::Complete,
            &ActionPayload::TargetCandidate(ActionTargetCandidate {
                target_section_ordinal: DecimalU64::new(candidate.section_ordinal()),
                target_object_ordinal: DecimalU64::new(candidate.ordinal()),
                target_type_code: candidate.type_code(),
                matches_target_kind: matches(candidate),
            }),
            &[],
        )?;
    }
    Ok(())
}

fn foreign_lookup(
    projection: &BankProjection<'_>,
    sites: &Sites,
    target_id: u32,
    target: Option<ForeignTarget>,
    matching: u64,
    complete: bool,
    writer: &MediaWriter<'_>,
) -> Result<ForeignLookup, MediaError> {
    Ok(match (target, matching, complete) {
        (None, _, _) => ForeignLookup::LocalOnly,
        (Some(target), 0, true) => {
            let evidence = origins::request(projection.input, sites, writer)?;
            let query_index = projection
                .directory
                .index
                .foreign_banks
                .request(target_id, target, evidence, writer)?;
            ForeignLookup::DirectorySearch {
                target,
                query_index,
            }
        }
        (Some(target), 0, false) => ForeignLookup::Unavailable {
            target,
            reason: ForeignUnavailable::IncompleteLocalInventory,
        },
        (Some(target), _, _) => ForeignLookup::LocalMatch { target },
    })
}
