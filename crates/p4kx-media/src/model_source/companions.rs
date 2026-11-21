// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::chunks::{ChunkDirectory, ivo};
use p4kx_extraction::model::companion_file_names;
use p4kx_extraction::{FieldPath, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins::{self, ModelOrigins};
use super::payload::ModelSourceFamily;
use crate::directory_index::model_companions::{normalized, source_level};
use crate::origin_contributions::OriginContributions;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::{
    MediaError, MediaKind, MediaWriter, OriginalIdentifier, Reference, SourceContext, Status,
    Subresource, origin_error,
};

pub(super) const NAMESPACE: &str = "model-companion";

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum CompanionRole {
    BaseGeometry,
    LodGeometry,
    LodModel,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CompanionPayload {
    role: CompanionRole,
    suffix: Option<String>,
    level: Option<u32>,
    canonical_reader_candidate: bool,
    #[schemars(schema_with = "crate::schema::reference_definition")]
    reference: Reference,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum CompanionRecord {
    Companion(CompanionPayload),
}

struct Candidate {
    path: String,
    evidence: OriginContributions,
}

pub(super) fn emit(
    context: &SourceContext<'_>,
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = context.entry.path();
    let Some(family) = ModelSourceFamily::of(path) else {
        return Ok(());
    };
    if !family.is_model()
        || source_level(path).is_some()
        || context
            .directory
            .index
            .archive
            .raw_path(context.entry.index())
            .is_some()
    {
        return Ok(());
    }
    let candidates = discover(context, file, origins, writer)?;
    if candidates.is_empty() {
        return Ok(());
    }
    let expected = expected_names(path);
    for (ordinal, candidate) in candidates.iter().enumerate() {
        write(context, ordinal, candidate, &expected, origins, writer)?;
    }
    Ok(())
}

fn discover(
    context: &SourceContext<'_>,
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<Candidate>, MediaError> {
    let path = context.entry.path();
    let siblings = context.directory.model_companions(path, writer)?;
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let observer = writer.origins.observer()?;
    for entry in siblings.entries() {
        writer.checkpoint()?;
        let entry = entry?;
        if seen.contains(&normalized(entry.path())) {
            continue;
        }
        let input = observer.entry(context.directory.index.archive, entry)?;
        let witness = ModelOrigins {
            input: &input,
            entry: entry.index(),
        };
        let evidence = witness.directory("modelCompanionObservedPath", writer)?;
        append(
            &mut candidates,
            &mut seen,
            entry.path().to_owned(),
            evidence,
        )?;
    }
    let base = format!("{path}m");
    if !seen.contains(&normalized(&base))
        && let Some(evidence) = announced_geometry(file, origins, writer)?
    {
        append(&mut candidates, &mut seen, base, evidence)?;
    }
    let physical = candidates.len();
    for ordinal in 0..physical {
        let candidate = candidates
            .get(ordinal)
            .ok_or_else(|| MediaError::Contract("original companion was removed".into()))?;
        if source_level(&candidate.path).is_some() {
            let path = counterpart(&candidate.path);
            let evidence = candidate.evidence.clone();
            append(&mut candidates, &mut seen, path, evidence)?;
        }
    }
    Ok(candidates)
}

fn announced_geometry(
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Option<OriginContributions>, MediaError> {
    let Some(index) = file
        .chunks()
        .iter()
        .position(|chunk| chunk.chunk_type() == ivo::MESH_INFO)
    else {
        return Ok(None);
    };
    let row = file
        .table_row(index)
        .ok_or_else(|| MediaError::Contract("model lost its authored mesh-info row".into()))?;
    let name = origins.directory("modelCompanionBasePath", writer)?;
    let announcement = origins.payload(
        row.kind,
        "modelMeshInfoChunk",
        SourceUseRole::Selector,
        writer,
    )?;
    let mut capture = writer.capture_origins();
    capture.include_original(&name)?;
    capture.include_original(&announcement)?;
    Ok(Some(capture.finish()))
}

fn append(
    candidates: &mut Vec<Candidate>,
    seen: &mut HashSet<String>,
    path: String,
    evidence: OriginContributions,
) -> Result<(), MediaError> {
    let normalized = normalized(&path);
    if !seen.contains(&normalized) {
        seen.try_reserve(1)
            .map_err(|cause| MediaError::Contract(cause.to_string()))?;
        candidates
            .try_reserve(1)
            .map_err(|cause| MediaError::Contract(cause.to_string()))?;
        seen.insert(normalized);
        candidates.push(Candidate { path, evidence });
    }
    Ok(())
}

fn counterpart(path: &str) -> String {
    let geometry = ModelSourceFamily::of(path).is_some_and(|kind| !kind.is_model());
    if geometry {
        path.get(..path.len() - 1).unwrap_or(path).to_owned()
    } else {
        format!("{path}m")
    }
}

fn expected_names(path: &str) -> HashSet<String> {
    companion_file_names(path)
        .into_iter()
        .map(|path| normalized(&path))
        .collect()
}

fn write(
    context: &SourceContext<'_>,
    ordinal: usize,
    candidate: &Candidate,
    expected: &HashSet<String>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    let path = &candidate.path;
    let source = &candidate.evidence;
    for field in ["role", "suffix", "level", "canonical_reader_candidate"] {
        origins::at(
            &mut prepared,
            &format!("/data/payload/{field}"),
            source.clone(),
            writer,
        )?;
    }
    let lookup = context.directory.lookup_input_observed(
        OriginalIdentifier::new_with_path(path),
        path,
        Some(MediaKind::Model),
        writer,
    )?;
    let reference = lookup.lookup.reference;
    let prefix = FieldPath::parse("/data/payload/reference")
        .map_err(|cause| origin_error::address(&cause))?;
    ReferenceOriginInputs {
        original: source,
        selector: source,
        inventory: &lookup.inventory,
        semantic: &origins.independent(),
        candidates: &lookup.evidence,
        read_selected: false,
        identifier: ReferenceIdentifier::Path,
        subresource: ReferenceSubresource::None,
    }
    .write(&mut prepared, &prefix, &reference, writer)?;
    let suffix = source_level(path);
    let role = role(path, suffix);
    prepared.write(
        writer,
        &Subresource::new(NAMESPACE, vec![ordinal as u64]),
        Status::Complete,
        &CompanionRecord::Companion(CompanionPayload {
            role,
            suffix: suffix.map(str::to_owned),
            level: suffix.and_then(|level| level.parse().ok()),
            canonical_reader_candidate: expected.contains(&normalized(path)),
            reference,
        }),
        &[],
    )
}

fn role(path: &str, suffix: Option<&str>) -> CompanionRole {
    if suffix.is_none() {
        CompanionRole::BaseGeometry
    } else if ModelSourceFamily::of(path).is_some_and(ModelSourceFamily::is_model) {
        CompanionRole::LodModel
    } else {
        CompanionRole::LodGeometry
    }
}
