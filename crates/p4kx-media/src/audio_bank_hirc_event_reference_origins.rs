// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankEventAction, BankHircObject};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

use super::EventSites;

pub(super) fn resolution(
    input: &MediaOriginInput,
    sites: &EventSites,
    action: &BankEventAction,
    complete: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let action_range = [action.action_id.range.start, action.action_id.range.end];
    let action_indices = sites.action_indices(action.ordinal);
    let authored = action_evidence(&prepared, sites, &action_range, &action_indices)?;
    prepared.indices(authored)?;
    let observed = prepared.capture(sites.base().into_iter().chain([
        action_word(&action_range),
        complete_directory("hircSections"),
        (
            OriginSite::new("bnkHircInventory", &[], "observedObjectEnvelopes"),
            SourceUseRole::Selector,
        ),
    ]))?;
    for field in [
        "/data/payload/observed_candidates",
        "/data/payload/matching_candidates",
        "/data/payload/candidates_complete",
    ] {
        prepared.field(field, observed.clone())?;
    }
    let outcome = if complete {
        observed
    } else {
        let mut capture = writer.capture_origins();
        capture.include(&observed, writer)?;
        capture.include(&OriginContributions::unknown(&input.owner), writer)?;
        capture.finish()
    };
    prepared.field("/data/payload/resolution", outcome)?;
    Ok(prepared.finish())
}

pub(super) fn candidate(
    input: &MediaOriginInput,
    sites: &EventSites,
    action: &BankEventAction,
    target: &BankHircObject,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let action_range = [action.action_id.range.start, action.action_id.range.end];
    let action_indices = sites.action_indices(action.ordinal);
    let target_sites = ObjectSites::new(target);
    let target_indices = target_sites.indices();
    let target_section = [target_indices[0]];
    let body = [target.body_offset()];
    let evidence = prepared.capture(sites.base().into_iter().chain([
        action_word(&action_range),
        target_sites.route(),
        (
            OriginSite::new("bnkHircObject", &body, "objectId"),
            SourceUseRole::Selector,
        ),
        (
            OriginSite::new("bnkHircObject", &body, "typeCode"),
            SourceUseRole::Payload,
        ),
    ]))?;
    prepared.indices(evidence.clone())?;
    let authored = action_evidence(&prepared, sites, &action_range, &action_indices)?;
    for ordinal in 0..action_indices.len() {
        prepared.field(&format!("/subresource/indices/{ordinal}"), authored.clone())?;
    }
    let section = prepared.capture([section_routing(&target_section)])?;
    let position = prepared.capture([target_sites.route()])?;
    prepared.field("/data/payload/target_section_ordinal", section)?;
    prepared.field("/data/payload/target_object_ordinal", position)?;
    prepared.field("/data/payload/target_type_code", evidence.clone())?;
    prepared.field("/data/payload/action_candidate", evidence)?;
    Ok(prepared.finish())
}

fn action_evidence(
    prepared: &BankOrigins<'_, '_, '_>,
    sites: &EventSites,
    range: &[u64; 2],
    indices: &[u64; 3],
) -> Result<OriginContributions, MediaError> {
    prepared.capture(sites.base().into_iter().chain([
        action_word(range),
        (
            OriginSite::new("bnkHircEvent", indices, "actionRouting"),
            SourceUseRole::Selector,
        ),
    ]))
}

const fn action_word(range: &[u64; 2]) -> EvidenceSite<'_> {
    (
        OriginSite::new("bnkHircField", range, "eventActionId"),
        SourceUseRole::Selector,
    )
}
