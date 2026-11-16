// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{
    AudioCandidate, AudioCandidateState, AudioIndex, AudioIssue, AudioSurvey,
};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, SourceCoordinateOmission, SourceDetails, SourceEncryption,
    SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

use crate::forge_origins::{EvidenceError, ForgeOrigins};

#[derive(Clone, Copy)]
pub(super) enum SurveyMode {
    Witness,
    Complete,
}

pub(super) fn capture(
    archive: &Archive,
    uses: &SourceUses,
    survey: Option<AudioSurvey<'_, '_>>,
    mode: SurveyMode,
) -> Result<FieldOrigin, EvidenceError> {
    let Some(survey) = survey else {
        return Ok(uses.origin(&[], false));
    };
    let directory = uses.observe_root_directory_paths(archive)?;
    let complete = survey.complete() && directory.is_some();
    let mut facts: Vec<_> = directory.into_iter().collect();
    let all_candidates = match mode {
        SurveyMode::Complete => {
            for candidate in survey.candidates() {
                facts.push(candidate_fact(uses, candidate)?);
            }
            true
        }
        SurveyMode::Witness => {
            let mut candidates = survey.candidates();
            let first = candidates.next();
            let second = candidates.next();
            let all_candidates = second.is_none();
            let witness = first
                .filter(encrypted_contributor)
                .or_else(|| {
                    second
                        .into_iter()
                        .chain(candidates)
                        .find(encrypted_contributor)
                })
                .or(first);
            if let Some(candidate) = witness {
                facts.push(candidate_fact(uses, candidate)?);
            }
            all_candidates
        }
    };
    Ok(uses.origin(&facts, complete && all_candidates))
}

fn encrypted_contributor(candidate: &AudioCandidate<'_>) -> bool {
    candidate.has_retained_input()
        && candidate.entry().stored_encryption() == SourceEncryption::Encrypted
}

fn candidate_fact(
    uses: &SourceUses,
    candidate: AudioCandidate<'_>,
) -> Result<SourceUse, EvidenceError> {
    let source = uses.observe_with_details(
        SourceInput::ArchiveView(candidate.entry()),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    let role = if candidate.has_retained_input() {
        SourceUseRole::Selector
    } else {
        SourceUseRole::Attempted
    };
    if candidate.state() == AudioCandidateState::CapacityLimited && candidate.has_retained_input() {
        return Ok(source.at_opaque(role, SourceCoordinateOmission::CaptureLimit)?);
    }
    if let Some(len) = candidate.decoded_bytes() {
        Ok(source.at(
            SourceLocation::Subresource {
                namespace: "entryDecodedBytes".into(),
                indices: vec![0, len],
                field: "audioIndexSurvey".into(),
            },
            role,
        )?)
    } else {
        Ok(source.at_opaque(
            SourceUseRole::Attempted,
            SourceCoordinateOmission::Unavailable,
        )?)
    }
}

pub(super) fn issues(
    archive: &Archive,
    index: &AudioIndex,
    output: &mut ForgeOrigins,
    listed: usize,
    mode: SurveyMode,
) -> Result<(), EvidenceError> {
    let mut rows = Vec::new();
    let mut count = Vec::new();
    let observed = match mode {
        SurveyMode::Witness => listed,
        SurveyMode::Complete => index.issues().len(),
    };
    for (position, issue) in index.issues().iter().take(observed).enumerate() {
        let (path, diagnostic) = issue_origins(archive, &output.uses, issue)?;
        let row = output
            .uses
            .origin_with_origins([&path, &diagnostic], true)?;
        count.push(row.clone());
        if position < listed {
            let address = format!("issues/{position}");
            output.put(&format!("{address}/path"), path)?;
            output.put(&format!("{address}/kind"), diagnostic.clone())?;
            output.put(&format!("{address}/message"), diagnostic)?;
            output.put(&address, row.clone())?;
            rows.push(row);
        }
    }
    let issues = output.uses.origin_with_origins(&rows, true)?;
    output.put("issues", issues)?;
    let total = output
        .uses
        .origin_with_origins(&count, observed == index.issues().len())?;
    output.put("issues_total", total)
}

fn issue_origins(
    archive: &Archive,
    uses: &SourceUses,
    issue: &AudioIssue,
) -> Result<(FieldOrigin, FieldOrigin), EvidenceError> {
    let Some(entry) = issue.source(archive) else {
        let unknown = uses.origin(&[], false);
        return Ok((unknown.clone(), unknown));
    };
    let complete = entry.ancestors().all(|step| step.is_some());
    let directory = uses.observe(
        SourceInput::ArchiveView(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let path = directory.at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        SourceUseRole::Payload,
    )?;
    let path = uses.origin(&[path], complete);
    let attempted = directory.for_scope(InputScope::Payload).at_opaque(
        SourceUseRole::Attempted,
        SourceCoordinateOmission::Unavailable,
    )?;
    Ok((path, uses.origin(&[attempted], false)))
}
