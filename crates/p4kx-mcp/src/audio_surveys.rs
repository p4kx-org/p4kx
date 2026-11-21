// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, AudioIssue, AudioSurvey};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, SourceCoordinateOmission, SourceInput, SourceLocation,
    SourceUseRole, SourceUses,
};

use crate::forge_origins::{EvidenceError, ForgeOrigins};

pub(super) use p4kx_assembly::audio::AudioSurveyProof as SurveyMode;

pub(super) fn capture(
    uses: &SourceUses,
    survey: Option<AudioSurvey<'_, '_>>,
    mode: SurveyMode,
) -> Result<FieldOrigin, EvidenceError> {
    match survey {
        Some(survey) => Ok(survey.origins(uses, mode)?),
        None => Ok(uses.origin(&[], false)),
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
