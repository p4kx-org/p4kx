// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::header::{self, ReceiptOwner};
use super::lock::{invalid, regular_file};
use crate::OriginArtifactName;

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum ArtifactTableRow {
    Artifact {
        path: Value,
        byte_length: String,
        sha256: String,
    },
    ArtifactTableEnd {
        artifacts: String,
    },
}

pub(super) fn lookup(receipt: &Path, replacement: Option<&Path>) -> io::Result<Option<bool>> {
    let Some(reader) = header::open_existing(receipt, ReceiptOwner::Directory)? else {
        return Ok(None);
    };
    let directory = receipt
        .parent()
        .ok_or_else(|| invalid("the origin directory receipt has no parent"))?;
    let mut names = HashSet::new();
    let mut intersects = false;
    let records = serde_json::Deserializer::from_reader(reader).into_iter::<ArtifactTableRow>();
    for record in records {
        match record.map_err(|_| invalid("the directory receipt has an invalid artifact table"))? {
            ArtifactTableRow::Artifact {
                path,
                byte_length,
                sha256,
            } => {
                if !canonical_count(&byte_length)
                    || sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
                {
                    return Err(invalid(
                        "the directory artifact has an invalid byte binding",
                    ));
                }
                let name = OriginArtifactName::decode_basename(path)?;
                if let Some(target) = replacement {
                    intersects |= may_address(directory, &name, target)?;
                }
                if !names.insert(name) {
                    return Err(invalid("the directory artifact table repeats a path"));
                }
            }
            ArtifactTableRow::ArtifactTableEnd { artifacts } => {
                if artifacts != names.len().to_string() {
                    return Err(invalid(
                        "the directory artifact count does not match the table",
                    ));
                }
                return Ok(Some(intersects));
            }
        }
    }
    Err(invalid(
        "the directory receipt has an incomplete artifact table",
    ))
}

fn canonical_count(text: &str) -> bool {
    text.parse::<u64>()
        .is_ok_and(|value| value.to_string() == text)
}

fn may_address(directory: &Path, name: &OsStr, replacement: &Path) -> io::Result<bool> {
    let original = directory.join(name);
    if original == replacement {
        return Ok(true);
    }
    if regular_file(&original)?.is_some() {
        return Ok(fs::canonicalize(original)? == replacement);
    }
    match (
        name.to_str(),
        replacement.file_name().and_then(OsStr::to_str),
    ) {
        (Some(left), Some(right)) if left.is_ascii() && right.is_ascii() => {
            Ok(left.eq_ignore_ascii_case(right))
        }
        _ => Ok(true),
    }
}
