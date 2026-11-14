// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_origin as origins;
use crate::audio_control_payload::AtlPayload;
use crate::audio_record_origins as records;
use crate::bank_candidate_index::Resolution;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{CandidateInventory, DirectoryContext, MediaError, MediaWriter, Status, Subresource};

use super::{Index, NameScope, Query, Site, Target, borrowed};

pub(crate) const RESOLUTION: &str = "audio-control-reference-resolution";
pub(crate) const CANDIDATE: &str = "audio-control-reference-candidate";

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlReferenceResolution {
    owner_entry_index: DecimalU64,
    owner_element_index: DecimalU64,
    owner_declaration_ordinal: DecimalU64,
    directory_complete: bool,
    candidates_complete: bool,
    observed_candidates: DecimalU64,
    blocked_documents: DecimalU64,
    resolution: Resolution,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlReferenceCandidate {
    #[serde(rename = "target_entry_index")]
    entry_index: DecimalU64,
    #[serde(rename = "target_element_index")]
    element_index: DecimalU64,
    #[serde(rename = "target_declaration_ordinal")]
    declaration_ordinal: DecimalU64,
}

struct Coverage {
    directory_complete: bool,
    blocked: u64,
    evidence: OriginContributions,
}

pub(crate) fn project(
    context: &DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let inventory = context
        .lookup
        .index
        .atl_controls
        .index
        .try_borrow()
        .map_err(|_| borrowed())?;
    if inventory.queries.is_empty() {
        return Ok(());
    }
    let owner =
        writer.origins.directory().cloned().ok_or_else(|| {
            MediaError::Contract("ATL reference search has no owning Archive".into())
        })?;
    let coverage = directory_coverage(context, &inventory, &owner, writer)?;
    for (ordinal, query) in inventory.queries.iter().enumerate() {
        writer.checkpoint()?;
        project_query(&inventory, &coverage, &owner, query, ordinal as u64, writer)?;
    }
    Ok(())
}

fn directory_coverage(
    context: &DirectoryContext<'_, '_, '_>,
    inventory: &Index,
    owner: &MediaOriginInput,
    writer: &MediaWriter<'_>,
) -> Result<Coverage, MediaError> {
    let directory_complete = matches!(
        context.lookup.scan_inventory(writer)?,
        CandidateInventory::Complete
    );
    let mut capture = writer.capture_origins();
    capture.include(
        &crate::origin_lookup::directory(Some(owner), writer)?,
        writer,
    )?;
    let mut blocked = 0;
    for entry in context.entries {
        context
            .lookup
            .input_state()?
            .budget
            .metadata(0, 1, writer.limits(), writer.checkpoint)?;
        if !wwise_path::is_atl_definition(entry.path()) {
            continue;
        }
        let input = writer.origins.input(context.lookup.index.archive, entry)?;
        capture.add(
            &input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-directory", &[], "path"),
            SourceUseRole::Selector,
            writer,
        )?;
        if let Some(document) = inventory.documents.get(&entry.index()) {
            capture.include(&document.evidence, writer)?;
        }
        blocked += u64::from(
            !inventory
                .documents
                .get(&entry.index())
                .is_some_and(|document| document.complete),
        );
    }
    Ok(Coverage {
        directory_complete,
        blocked,
        evidence: capture.finish(),
    })
}

fn project_query(
    inventory: &Index,
    directory: &Coverage,
    owner: &MediaOriginInput,
    query: &Query,
    index: u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let targets = inventory
        .targets
        .get(&query.key)
        .and_then(|names| names.get(query.name.as_str()))
        .map_or(&[][..], Vec::as_slice);
    let coverage = query_coverage(inventory, directory, query.key.scope)?;
    let prepared = query_origins(owner, query, &coverage, writer)?;
    let observed = targets.len() as u64;
    let [element, declaration] = query.source.indices;
    prepared.write(
        writer,
        &Subresource::new(RESOLUTION, vec![index]),
        Status::Complete,
        &AtlPayload::ReferenceResolution(AtlReferenceResolution {
            owner_entry_index: DecimalU64::new(query.source.entry as u64),
            owner_element_index: DecimalU64::new(element),
            owner_declaration_ordinal: DecimalU64::new(declaration),
            directory_complete: directory.directory_complete,
            candidates_complete: coverage.complete,
            observed_candidates: DecimalU64::new(observed),
            blocked_documents: DecimalU64::new(coverage.blocked),
            resolution: Resolution::from_inventory(coverage.complete, observed),
        }),
        &[],
    )?;
    for (ordinal, target) in targets.iter().enumerate() {
        writer.checkpoint()?;
        candidate(owner, query, target, [index, ordinal as u64], writer)?;
    }
    Ok(())
}

struct QueryCoverage<'source> {
    complete: bool,
    blocked: u64,
    evidence: &'source OriginContributions,
}

fn query_coverage<'source>(
    inventory: &'source Index,
    directory: &'source Coverage,
    scope: NameScope,
) -> Result<QueryCoverage<'source>, MediaError> {
    Ok(match scope {
        NameScope::Directory => QueryCoverage {
            complete: directory.directory_complete && directory.blocked == 0,
            blocked: directory.blocked,
            evidence: &directory.evidence,
        },
        NameScope::Preload { entry, .. } => {
            let document = inventory.documents.get(&entry).ok_or_else(|| {
                MediaError::Contract("ATL preload search has no observed owner".into())
            })?;
            QueryCoverage {
                complete: document.complete,
                blocked: u64::from(!document.complete),
                evidence: &document.evidence,
            }
        }
    })
}

fn query_origins(
    owner: &MediaOriginInput,
    query: &Query,
    coverage: &QueryCoverage<'_>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(&query.evidence, writer)?;
    capture.include(coverage.evidence, writer)?;
    let known = capture.finish();
    let mut prepared = origins::prepared(owner, known.clone(), writer)?;
    for path in [
        "/data/payload/owner_element_index",
        "/data/payload/owner_declaration_ordinal",
    ] {
        records::field(&mut prepared, path, query.evidence.clone(), writer)?;
    }
    records::field(
        &mut prepared,
        "/data/payload/owner_entry_index",
        entry_index(&query.input, writer)?,
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/directory_complete",
        crate::origin_lookup::directory(Some(owner), writer)?,
        writer,
    )?;
    if !coverage.complete {
        let mut incomplete = writer.capture_origins();
        incomplete.include(&known, writer)?;
        incomplete.include(&OriginContributions::unknown(&owner.owner), writer)?;
        records::field(
            &mut prepared,
            "/data/payload/resolution",
            incomplete.finish(),
            writer,
        )?;
    }
    Ok(prepared)
}

fn candidate(
    owner: &MediaOriginInput,
    query: &Query,
    target: &Target,
    indices: [u64; 2],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Site {
        entry,
        indices: [element, declaration],
        position,
    } = target.site;
    let proof = origins::capture(
        &target.input,
        position,
        "declaration",
        SourceUseRole::Selector,
        writer,
    )?;
    let mut link = writer.capture_origins();
    link.include(&query.evidence, writer)?;
    link.include(&proof, writer)?;
    let mut prepared = origins::prepared(owner, link.finish(), writer)?;
    records::field(
        &mut prepared,
        "/data/payload/target_entry_index",
        entry_index(&target.input, writer)?,
        writer,
    )?;
    for path in [
        "/data/payload/target_element_index",
        "/data/payload/target_declaration_ordinal",
    ] {
        records::field(&mut prepared, path, proof.clone(), writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(CANDIDATE, indices.to_vec()),
        Status::Complete,
        &AtlPayload::ReferenceCandidate(AtlReferenceCandidate {
            entry_index: DecimalU64::new(entry as u64),
            element_index: DecimalU64::new(element),
            declaration_ordinal: DecimalU64::new(declaration),
        }),
        &[],
    )
}

fn entry_index(
    input: &MediaOriginInput,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-directory", &[], "entry_index"),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}
