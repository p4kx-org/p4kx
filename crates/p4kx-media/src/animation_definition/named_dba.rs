// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "named_dba_sources.rs"]
mod sources;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::cryxml::SourceElement;
use schemars::JsonSchema;
use serde::Serialize;

use self::sources::{Bank, ObservedMatch};
use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::projection::{DefinitionProjection, Failure};
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaKind, MediaWriter, SourceRange, Status, Subresource};

#[derive(Default)]
pub(super) struct Databases {
    declarations: Vec<Bank>,
}

pub(super) struct NamedSelection<'a> {
    pub(super) authored_path: &'a str,
    pub(super) archive_path: &'a str,
    pub(super) original: &'a OriginContributions,
    pub(super) selector: &'a OriginContributions,
}

impl Databases {
    pub(super) fn read(
        list: SourceElement<'_>,
        projection: &DefinitionProjection<'_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, Failure> {
        if !writer.has_kind(MediaKind::Animation) {
            return Ok(Self::default());
        }
        Ok(Self {
            declarations: sources::read(list, projection, writer)?,
        })
    }

    pub(super) fn write(
        &self,
        selection: &NamedSelection<'_>,
        projection: &DefinitionProjection<'_, '_>,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        if self.declarations.is_empty() {
            return Ok(());
        }
        let mut prepared = origins::record(projection.input, selection.original.clone(), writer)?;
        records::field(
            &mut prepared,
            "/data/payload/archive_path",
            selection.selector.clone(),
            writer,
        )?;
        let path_key = selection.archive_path.to_ascii_lowercase();
        let mut projection_state = BindingProjection::new(selection.selector, writer)?;
        self.project(
            &path_key,
            projection,
            &mut projection_state,
            &mut prepared,
            writer,
        )?;
        let binding = Binding {
            authored_path: selection.authored_path,
            archive_path: selection.archive_path,
            state: BindingState::of(
                projection_state.complete,
                projection_state.observed_candidates,
            ),
            candidates_complete: projection_state.complete,
            observed_candidates: DecimalU64::new(projection_state.observed_candidates),
            databases: projection_state.databases,
            candidates: projection_state.candidates,
        };
        let disposition = projection_state.disposition.finish();
        for field in ["state", "candidates_complete", "observed_candidates"] {
            records::field(
                &mut prepared,
                &format!("/data/payload/{field}"),
                disposition.clone(),
                writer,
            )?;
        }
        prepared.write(
            writer,
            &Subresource::new(wire::NAMED_DBA_BINDING, indices.to_vec()),
            Status::Complete,
            &DefinitionPayload::NamedDbaBinding(binding),
            &[],
        )?;
        Ok(())
    }

    fn project<'a>(
        &'a self,
        path_key: &str,
        projection: &DefinitionProjection<'_, '_>,
        result: &mut BindingProjection<'a>,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let budget = &projection.context.directory.input_state()?.budget;
        for (index, bank) in self.declarations.iter().enumerate() {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            result.bank(bank, index, prepared, writer)?;
            for matched in bank.state.matches(path_key) {
                budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
                result.matched(&matched, &bank.path_evidence, index, prepared, writer)?;
            }
        }
        Ok(())
    }
}

struct BindingProjection<'a> {
    complete: bool,
    observed_candidates: u64,
    databases: Vec<BankDisposition<'a>>,
    candidates: Vec<ClipCandidate<'a>>,
    disposition: crate::origin_capture::OriginCapture,
}

impl<'a> BindingProjection<'a> {
    fn new(selector: &OriginContributions, writer: &MediaWriter<'_>) -> Result<Self, Failure> {
        let mut disposition = writer.capture_origins();
        disposition.include(selector, writer)?;
        Ok(Self {
            complete: true,
            observed_candidates: 0,
            databases: Vec::new(),
            candidates: Vec::new(),
            disposition,
        })
    }

    fn bank(
        &mut self,
        bank: &'a Bank,
        index: usize,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let base = format!("/data/payload/databases/{index}");
        for (field, origin) in [
            ("authored_path", &bank.path_evidence),
            ("state", &bank.evidence),
        ] {
            records::field(prepared, &format!("{base}/{field}"), origin.clone(), writer)?;
        }
        self.disposition.include(&bank.evidence, writer)?;
        let state = bank.state.state();
        self.complete &= matches!(state, DatabaseState::Enumerated | DatabaseState::Missing);
        self.databases.push(BankDisposition {
            authored_path: bank.path.as_deref(),
            state,
        });
        Ok(())
    }

    fn matched(
        &mut self,
        matched: &ObservedMatch<'a>,
        declaration: &OriginContributions,
        index: usize,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.observed_candidates = self
            .observed_candidates
            .checked_add(1)
            .ok_or_else(crate::origin_error::counter)?;
        if self.candidates.len() >= writer.limits().reference_candidates as usize {
            self.complete = false;
            return Ok(());
        }
        let proof = matched.proof(writer)?;
        self.disposition.include(&proof.name, writer)?;
        let base = format!("/data/payload/candidates/{}", self.candidates.len());
        for (field, origin) in [
            ("declaration_ordinal", declaration),
            ("entry", &proof.entry),
            ("clip_ordinal", &proof.metadata),
            ("authored_name", &proof.name),
            ("name_range", &proof.name),
            ("metadata_range", &proof.metadata),
        ] {
            records::field(prepared, &format!("{base}/{field}"), origin.clone(), writer)?;
        }
        self.candidates.push(matched.candidate(index as u64));
        Ok(())
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum BindingState {
    BankMatch,
    AmbiguousBankMatch,
    LooseLookup,
    Incomplete,
}

impl BindingState {
    const fn of(complete: bool, candidates: u64) -> Self {
        match (complete, candidates) {
            (false, _) => Self::Incomplete,
            (true, 0) => Self::LooseLookup,
            (true, 1) => Self::BankMatch,
            (true, _) => Self::AmbiguousBankMatch,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding<'a> {
    authored_path: &'a str,
    archive_path: &'a str,
    state: BindingState,
    candidates_complete: bool,
    observed_candidates: DecimalU64,
    databases: Vec<BankDisposition<'a>>,
    candidates: Vec<ClipCandidate<'a>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BankDisposition<'a> {
    authored_path: Option<&'a str>,
    state: DatabaseState,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum DatabaseState {
    Enumerated,
    Incomplete,
    InvalidIdentifier,
    Missing,
    Ambiguous,
    NotSelected,
    Unavailable,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClipCandidate<'a> {
    declaration_ordinal: DecimalU64,
    #[schemars(schema_with = "identity_definition")]
    entry: &'a crate::Identity,
    clip_ordinal: DecimalU64,
    authored_name: &'a str,
    name_range: SourceRange,
    metadata_range: SourceRange,
}

fn identity_definition(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"$ref": "#/$defs/Identity"})
}
