// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
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

#[derive(Clone, Copy)]
pub(super) enum ReplacementPath<'a> {
    File(&'a Path),
    SymbolicLink(&'a Path),
}

impl ReplacementPath<'_> {
    fn may_address(
        self,
        directory: &Path,
        name: &OsStr,
        lexical_names: &mut Option<HashSet<OsString>>,
    ) -> io::Result<bool> {
        match self {
            Self::File(target) => may_address(directory, name, target),
            Self::SymbolicLink(target) => may_address_link(directory, name, target, lexical_names),
        }
    }
}

pub(super) fn lookup(
    receipt: &Path,
    replacement: Option<ReplacementPath<'_>>,
) -> io::Result<Option<bool>> {
    let Some(reader) = header::open_existing(receipt, ReceiptOwner::Directory)? else {
        return Ok(None);
    };
    let directory = receipt
        .parent()
        .ok_or_else(|| invalid("the origin directory receipt has no parent"))?;
    let mut names = HashSet::new();
    let mut intersects = false;
    let mut lexical_names = None;
    let records = serde_json::Deserializer::from_reader(reader).into_iter::<ArtifactTableRow>();
    for record in records {
        match record.map_err(|error| {
            if error.is_io() {
                io::Error::from(error)
            } else {
                invalid("the directory receipt has an invalid artifact table")
            }
        })? {
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
                    intersects |= target.may_address(directory, &name, &mut lexical_names)?;
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
    match regular_file(&original) {
        Ok(Some(_)) => return Ok(fs::canonicalize(original)? == replacement),
        Ok(None) => {}
        Err(error)
            if error.kind() == io::ErrorKind::InvalidInput && error.raw_os_error().is_none() =>
        {
            return Ok(true);
        }
        Err(error) => return Err(error),
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

fn may_address_link(
    directory: &Path,
    name: &OsStr,
    replacement: &Path,
    lexical_names: &mut Option<HashSet<OsString>>,
) -> io::Result<bool> {
    let original = directory.join(name);
    if original == replacement {
        return Ok(true);
    }
    match fs::symlink_metadata(original) {
        Ok(metadata) if metadata.file_type().is_symlink() => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }
    if lexical_names.is_none() {
        *lexical_names = Some(
            fs::read_dir(directory)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<io::Result<_>>()?,
        );
    }
    let disjoint = lexical_names.as_ref().is_some_and(|names| {
        names.contains(name)
            && replacement
                .file_name()
                .is_some_and(|name| names.contains(name))
    });
    Ok(!disjoint)
}
