// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::Archive;
use p4kx_media::{
    ExportOptions, ExportOutcome, Index, MediaError, MediaKind, MediaRecord, ProviderRegistry,
    ShaderProvider,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn archive(
    root: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok(Archive::open(path)?)
}

pub(super) fn registry(provider: ShaderProvider) -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![Box::new(provider)])
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Shader],
        ..ExportOptions::default()
    }
}

pub(super) fn published(outcome: ExportOutcome) -> Result<Index, Box<dyn std::error::Error>> {
    if let ExportOutcome::Published { index, .. } = outcome {
        Ok(*index)
    } else {
        Err("expected complete shader generation".into())
    }
}

pub(super) struct Diagnostic {
    pub(super) index: Index,
    pub(super) directory: PathBuf,
}

pub(super) fn incomplete(outcome: ExportOutcome) -> Result<Diagnostic, Box<dyn std::error::Error>> {
    if let ExportOutcome::Incomplete { index, diagnostic } = outcome {
        let directory = diagnostic
            .parent()
            .ok_or("diagnostic index has no artifact directory")?
            .to_path_buf();
        Ok(Diagnostic {
            index: *index,
            directory,
        })
    } else {
        Err("expected incomplete shader diagnostic generation".into())
    }
}

pub(super) fn records(
    output: &Path,
    index: &Index,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    fs::read_to_string(output.join(&index.artifact_base).join("shader.jsonl"))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub(super) fn replace(json: &mut Value, pointer: &str, value: Value) -> TestResult {
    *json
        .pointer_mut(pointer)
        .ok_or_else(|| format!("missing fixture field {pointer}"))? = value;
    Ok(())
}

pub(super) fn projection_loss(error: &MediaError) -> bool {
    match error {
        MediaError::ProjectionLoss {
            kind: MediaKind::Shader,
            ..
        } => true,
        MediaError::Source { cause, .. } => projection_loss(cause),
        _ => false,
    }
}
