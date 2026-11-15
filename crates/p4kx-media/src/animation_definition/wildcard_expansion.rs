// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, Entry, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::projection::DefinitionProjection;
use super::wildcard_match::{Family, Rule, Unexpanded};
use crate::audio_record_origins as records;
use crate::lookup_work::LookupWork;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::source_selection::EntrySelection;
use crate::{CandidateInventory, MediaError, MediaWriter, Status, Subresource};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationWildcardExpansion")]
pub(super) struct Expansion {
    state: State,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
enum State {
    Scan(Scan),
    Unexpanded(Unexpanded),
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Scan {
    Expanded,
    IncompleteInventory,
    RepairedSource,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationWildcardCandidate")]
pub(super) struct Candidate<'source> {
    alias: String,
    archive_path: &'source str,
    entry_index: DecimalU64,
    family: Family,
    selection: Selection,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Selection {
    Included,
    Excluded,
    Unmatched,
}

impl From<EntrySelection> for Selection {
    fn from(selection: EntrySelection) -> Self {
        match selection {
            EntrySelection::Included => Self::Included,
            EntrySelection::Excluded => Self::Excluded,
            EntrySelection::Unmatched => Self::Unmatched,
        }
    }
}

pub(super) fn write(
    projection: &DefinitionProjection<'_, '_>,
    rule: Result<Rule<'_>, Unexpanded>,
    indices: [u64; 2],
    selectors: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = projection.context.directory;
    let mut proof = writer.capture_origins();
    proof.include_original(selectors)?;
    let state = match (projection.repaired_strings, rule) {
        (true, _) => State::Scan(Scan::RepairedSource),
        (false, Ok(rule)) => {
            let inventory = directory.scan_inventory(writer)?;
            let owner = writer
                .origins
                .directory()
                .ok_or_else(crate::origin_error::foreign)?;
            proof.add(
                owner,
                InputScope::Payload,
                OriginSite::new("archive-directory", &[], "animation-wildcard-inventory"),
                SourceUseRole::Selector,
                writer,
            )?;
            let exact_paths = write_matches(projection, &rule, indices, selectors, writer)?;
            State::Scan(
                if exact_paths && matches!(inventory, CandidateInventory::Complete) {
                    Scan::Expanded
                } else {
                    Scan::IncompleteInventory
                },
            )
        }
        (false, Err(reason)) => State::Unexpanded(reason),
    };
    origins::record(projection.input, proof.finish(), writer)?.write(
        writer,
        &Subresource::new(wire::WILDCARD_EXPANSION, indices.to_vec()),
        Status::Complete,
        &DefinitionPayload::WildcardExpansion(Expansion { state }),
        &[],
    )
}

fn write_matches(
    projection: &DefinitionProjection<'_, '_>,
    rule: &Rule<'_>,
    indices: [u64; 2],
    selectors: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<bool, MediaError> {
    let directory = projection.context.directory;
    let limits = writer.limits();
    let work = LookupWork {
        limits,
        checkpoint: writer.checkpoint,
        budget: directory.budget,
        source_allowance: directory.allowance(limits.sources)?,
    };
    let mut ordinal = 0_u64;
    directory
        .index
        .visit_animation_wildcard(rule, &work, |entry, captured| {
            let [element, declaration] = indices;
            let site = [element, declaration, ordinal];
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(crate::origin_error::counter)?;
            write_candidate(projection, rule, (entry, captured), site, selectors, writer)
        })
}

fn write_candidate(
    projection: &DefinitionProjection<'_, '_>,
    rule: &Rule<'_>,
    (entry, captured): (&Entry, &str),
    site: [u64; 3],
    selectors: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = projection.context.directory;
    let index = entry.index() as u64;
    let input = writer.origins.input(directory.index.archive, entry)?;
    let mut path = writer.capture_origins();
    path.add(
        &input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-entry", &[index], "path"),
        SourceUseRole::Selector,
        writer,
    )?;
    let path = path.finish();
    let mut evidence = writer.capture_origins();
    evidence.include_original(selectors)?;
    evidence.include_original(&path)?;
    let mut prepared = origins::record(projection.input, evidence.finish(), writer)?;
    for field in [
        "/data/payload/archive_path",
        "/data/payload/entry_index",
        "/data/payload/family",
    ] {
        records::field(&mut prepared, field, path.clone(), writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(wire::WILDCARD_CANDIDATE, site.to_vec()),
        Status::Complete,
        &DefinitionPayload::WildcardCandidate(Candidate {
            alias: rule.alias(captured),
            archive_path: entry.path(),
            entry_index: DecimalU64::new(index),
            family: rule.family(),
            selection: directory.reference_selection(entry).into(),
        }),
        &[],
    )
}
