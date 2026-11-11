// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlPayload};
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{
    CandidateInventory, CandidateState, DirectoryLookup, MediaError, MediaKind, MediaWriter,
    NotSelectedReason, ReferenceUnavailable, Status, Subresource,
};

use super::{
    AtlReference, AtlReferenceLink, AtlReferenceTarget, AtlReferenceUnavailable, NameDeclaration,
};

pub(crate) const PATH_CANDIDATE: &str = "audio-control-path-candidate";

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum AtlFileKind {
    Bank,
    ExternalMedia,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlPathCandidate {
    target_entry_index: DecimalU64,
    state: CandidateState,
    key_required: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum PathDisposition {
    Resolved,
    Missing,
    Ambiguous,
    NotSelected { reason: NotSelectedReason },
    Unavailable { reason: ReferenceUnavailable },
}

pub(super) fn project(
    directory: &DirectoryLookup<'_>,
    input: &MediaOriginInput,
    declaration: NameDeclaration<'_>,
    kind: AtlFileKind,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = origins::capture(
        input,
        declaration.site.position,
        "declaration",
        SourceUseRole::Selector,
        writer,
    )?;
    let mut prepared = origins::prepared(input, original.clone(), writer)?;
    let link = if let Some(reason) = unavailable_path(declaration.name, kind) {
        writer.start_reference()?;
        AtlReferenceLink::Unavailable { reason }
    } else {
        let (link, evidence) = lookup(directory, input, declaration, kind, &original, writer)?;
        if matches!(
            link,
            AtlReferenceLink::PathSearch {
                candidates_complete: false,
                ..
            }
        ) {
            let mut incomplete = writer.capture_origins();
            incomplete.include(&evidence, writer)?;
            incomplete.include(&OriginContributions::unknown(&input.owner), writer)?;
            records::subtree(
                &mut prepared,
                "/data/payload/link/resolution",
                incomplete.finish(),
                writer,
            )?;
        }
        records::subtree(&mut prepared, "/data/payload/link", evidence, writer)?;
        link
    };
    prepared.write(
        writer,
        &Subresource::new(wire::REFERENCE, declaration.site.indices.to_vec()),
        Status::Complete,
        &AtlPayload::Reference(AtlReference {
            target: AtlReferenceTarget::File {
                kind,
                path: declaration.name,
            },
            link,
        }),
        &[],
    )
}

fn unavailable_path(path: &str, kind: AtlFileKind) -> Option<AtlReferenceUnavailable> {
    let mut parts = path.split(['/', '\\']);
    let first = parts.next().unwrap_or_default();
    if !safe_component(first) || parts.clone().any(|part| !safe_component(part)) {
        return Some(AtlReferenceUnavailable::UnsafePath);
    }
    if !first.eq_ignore_ascii_case("data")
        || parts.next().is_none()
        || (matches!(kind, AtlFileKind::Bank)
            && !path
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bnk")))
    {
        return Some(AtlReferenceUnavailable::UnprovedTargetPath);
    }
    None
}

fn safe_component(part: &str) -> bool {
    !part.is_empty()
        && !matches!(part, "." | "..")
        && !part.contains([':', '\0'])
        && !part.ends_with([' ', '.'])
}

fn lookup(
    directory: &DirectoryLookup<'_>,
    input: &MediaOriginInput,
    declaration: NameDeclaration<'_>,
    kind: AtlFileKind,
    original: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<(AtlReferenceLink, OriginContributions), MediaError> {
    let mut count = 0;
    let mut last = None;
    let mut evidence = writer.capture_origins();
    evidence.include(original, writer)?;
    evidence.include(
        &crate::origin_lookup::directory(writer.origins.directory(), writer)?,
        writer,
    )?;
    let target_kind = match kind {
        AtlFileKind::Bank => Some(MediaKind::Audio),
        AtlFileKind::ExternalMedia => None,
    };
    let inventory = directory.visit_path_candidates(
        declaration.name,
        target_kind,
        writer,
        |entry, state, writer| {
            let target = writer.origins.input(directory.index.archive, entry)?;
            let proof = candidate_origin(&target, writer)?;
            evidence.include(&proof, writer)?;
            let [element, declaration] = declaration.site.indices;
            write_candidate(
                input,
                original,
                &proof,
                [element, declaration, count],
                AtlPathCandidate {
                    target_entry_index: DecimalU64::new(entry.index() as u64),
                    state,
                    key_required: entry.is_encrypted() && !directory.index.archive.has_key(),
                },
                writer,
            )?;
            last = Some(state);
            count += 1;
            Ok(())
        },
    )?;
    let complete = matches!(inventory, CandidateInventory::Complete);

    Ok((
        AtlReferenceLink::PathSearch {
            resolution: disposition(inventory, count, last),
            candidates_complete: complete,
            observed_candidates: DecimalU64::new(count),
        },
        evidence.finish(),
    ))
}

fn candidate_origin(
    input: &MediaOriginInput,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut proof = writer.capture_origins();
    for field in ["entry_index", "path", "encryption"] {
        proof.add(
            input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-directory", &[], field),
            SourceUseRole::Selector,
            writer,
        )?;
    }
    Ok(proof.finish())
}

fn write_candidate(
    input: &MediaOriginInput,
    original: &OriginContributions,
    target: &OriginContributions,
    indices: [u64; 3],
    candidate: AtlPathCandidate,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut link = writer.capture_origins();
    link.include(original, writer)?;
    link.include(target, writer)?;
    let mut prepared = records::prepare_for(input, OriginProfile::AudioControl)?;
    records::indices(&mut prepared, link.finish(), writer)?;
    records::subtree(&mut prepared, "/data/payload", target.clone(), writer)?;
    prepared.write(
        writer,
        &Subresource::new(PATH_CANDIDATE, indices.to_vec()),
        Status::Complete,
        &AtlPayload::PathCandidate(candidate),
        &[],
    )
}

const fn disposition(
    inventory: CandidateInventory,
    count: u64,
    last: Option<CandidateState>,
) -> PathDisposition {
    match (inventory, count, last) {
        (CandidateInventory::Unavailable(reason), _, _) => PathDisposition::Unavailable { reason },
        (_, 0, _) => PathDisposition::Missing,
        (_, 1, Some(CandidateState::Selected)) => PathDisposition::Resolved,
        (_, 1, Some(CandidateState::NotSelected { reason })) => {
            PathDisposition::NotSelected { reason }
        }
        (_, 1, Some(CandidateState::Unavailable { reason })) => {
            PathDisposition::Unavailable { reason }
        }
        _ => PathDisposition::Ambiguous,
    }
}
