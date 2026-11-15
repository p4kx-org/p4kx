// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, Entry, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::clip_candidates::{AliasKind, AliasProofs, Fact, Site, Target};
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
    proofs: (&OriginContributions, &OriginContributions),
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = projection.context.directory;
    let mut proof = writer.capture_origins();
    proof.include_original(proofs.0)?;
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
            let exact_paths = write_matches(projection, &rule, indices, proofs, writer)?;
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
    proofs: (&OriginContributions, &OriginContributions),
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
            write_candidate(projection, rule, (entry, captured), site, proofs, writer)
        })
}

fn write_candidate(
    projection: &DefinitionProjection<'_, '_>,
    rule: &Rule<'_>,
    (entry, captured): (&Entry, &str),
    site: [u64; 3],
    (selectors, site_proof): (&OriginContributions, &OriginContributions),
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = projection.context.directory;
    let index = entry.index() as u64;
    let path = origins::physical_path(directory, entry, writer)?;
    let mut evidence = writer.capture_origins();
    evidence.include_original(selectors)?;
    evidence.include_original(&path)?;
    let evidence = evidence.finish();
    let mut prepared = origins::record(projection.input, evidence.clone(), writer)?;
    for field in [
        "/data/payload/archive_path",
        "/data/payload/entry_index",
        "/data/payload/family",
    ] {
        records::field(&mut prepared, field, path.clone(), writer)?;
    }
    let alias = rule.alias(captured);
    let selection = directory.reference_selection(entry);
    if selection == EntrySelection::Included && directory.uses_definition_scope() {
        observe_candidate(
            projection,
            (entry.index(), rule.family()),
            site,
            &alias,
            AliasProofs {
                occurrence: &evidence,
                name: &evidence,
                site: site_proof,
                target: &path,
                target_clip: None,
            },
            writer,
        )?;
    }
    prepared.write(
        writer,
        &Subresource::new(wire::WILDCARD_CANDIDATE, site.to_vec()),
        Status::Complete,
        &DefinitionPayload::WildcardCandidate(Candidate {
            alias,
            archive_path: entry.path(),
            entry_index: DecimalU64::new(index),
            family: rule.family(),
            selection: selection.into(),
        }),
        &[],
    )
}

fn observe_candidate(
    projection: &DefinitionProjection<'_, '_>,
    (entry, family): (usize, Family),
    [element, declaration, ordinal]: [u64; 3],
    name: &str,
    origins: AliasProofs<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let target = match family {
        Family::Caf => Target::Caf { entry },
        Family::BlendSpace => Target::BlendSpace { entry },
        Family::CombinedSpace => Target::CombinedSpace { entry },
    };
    projection
        .context
        .directory
        .index
        .animation_clip_candidates
        .observe(
            Fact::Alias {
                site: Site {
                    entry: projection.context.entry.index(),
                    indices: [element, declaration],
                },
                ordinal,
                kind: AliasKind::Wildcard,
                name,
                target,
                origins,
            },
            writer,
        )
}
