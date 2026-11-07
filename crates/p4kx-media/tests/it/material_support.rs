// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_media::{
    ExportOptions, ExportOutcome, Index, MaterialProvider, MediaError, MediaKind, MediaRecord,
    ProviderRegistry,
};
use p4kx_testkit::TestEntry;

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn archive(
    root: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Archive, Box<dyn std::error::Error>> {
    super::support::archive_with(root, entries)
}

pub(super) fn registry(provider: MaterialProvider) -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![Box::new(provider)])
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Material],
        ..ExportOptions::default()
    }
}

pub(super) fn published(outcome: ExportOutcome) -> Result<Index, Box<dyn std::error::Error>> {
    super::support::published(outcome)
}

pub(super) fn records(
    output: &Path,
    index: &Index,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    super::support::records(output, index, "material.jsonl")
}

pub(super) fn diagnostic_records(
    outcome: ExportOutcome,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("expected incomplete material generation".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic directory")?;
    fs::read_to_string(directory.join("material.jsonl"))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub(super) fn projection_loss(error: &MediaError) -> bool {
    match error {
        MediaError::ProjectionLoss {
            kind: MediaKind::Material,
            ..
        } => true,
        MediaError::Source { cause, .. } => projection_loss(cause),
        _ => false,
    }
}
