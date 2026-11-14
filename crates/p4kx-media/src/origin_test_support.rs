// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use crate::{ArchiveProvider, ExportOptions, Index, ProviderRegistry, ShaderProvider};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(crate) fn export(
    folder: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<(Archive, PathBuf, Index), Box<dyn std::error::Error>> {
    let path = folder.join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(p4kx_testkit::test_key())))?;
    let output = folder.join("media");
    let registry = ProviderRegistry::try_new(vec![
        Box::new(ArchiveProvider),
        Box::new(ShaderProvider::default()),
    ])?;
    let index = match crate::export_with(&archive, &output, &ExportOptions::default(), &registry)? {
        crate::ExportOutcome::Published { index, .. } => *index,
        _ => return Err("expected complete synthetic origin generation".into()),
    };
    Ok((archive, output, index))
}

pub(crate) fn rows(
    output: &Path,
    index: &Index,
    relative: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    fs::read_to_string(output.join(&index.artifact_base).join(relative))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub(crate) fn ordinal(
    output: &Path,
    index: &Index,
    kind: &str,
    path: &str,
    namespace: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    rows(output, index, &format!("{kind}.jsonl"))?
        .iter()
        .position(|row| {
            row.pointer("/source/identity/entry/path")
                .and_then(Value::as_str)
                == Some(path)
                && row
                    .pointer("/subresource/namespace")
                    .and_then(Value::as_str)
                    == Some(namespace)
        })
        .ok_or_else(|| "actual payload occurrence".into())
}

pub(crate) fn field(
    output: &Path,
    index: &Index,
    kind: &str,
    ordinal: usize,
    pointer: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let ordinal = ordinal.to_string();
    rows(output, index, "field-origins.jsonl")?
        .into_iter()
        .find(|row| {
            row.get("type").and_then(Value::as_str) == Some("field")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some(kind)
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && row.get("field").and_then(Value::as_str) == Some(pointer)
        })
        .ok_or_else(|| "actual immediate field origin".into())
}

pub(crate) fn marker(field: &Value) -> Option<bool> {
    field
        .get("containsEncryptedSource")
        .and_then(Value::as_bool)
}

pub(crate) fn complete(field: &Value) -> Option<bool> {
    field.get("sourceSetComplete").and_then(Value::as_bool)
}
pub(crate) fn has_limit(mut error: &(dyn std::error::Error + 'static), expected: &str) -> bool {
    loop {
        if error.downcast_ref::<crate::MediaError>().is_some_and(
            |error| matches!(error, crate::MediaError::Limit { budget, .. } if *budget == expected),
        ) {
            return true;
        }
        match error.source() {
            Some(source) => error = source,
            None => return false,
        }
    }
}
