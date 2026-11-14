// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryIssue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One issue in the Archive reader's bounded directory sample.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DirectoryIssue {
    /// Physical index for an Entry issue; absent for a global Archive marker.
    pub entry_index: Option<usize>,
    /// Stable namespaced diagnosis code.
    pub code: String,
    /// Whether the issue affects the Archive inventory rather than one Entry.
    pub archive_wide: bool,
    /// Reader-provided technical description.
    pub detail: String,
    /// Raw encryption value when available.
    pub encryption_value: Option<u16>,
    /// Raw encryption or hash block length when available.
    pub block_size: Option<u16>,
}

pub(crate) fn project(entry_index: usize, issue: &EntryIssue) -> DirectoryIssue {
    let (code, encryption_value, block_size) = match issue {
        EntryIssue::DataOutsideFile => ("data-outside-file", None, None),
        EntryIssue::UnknownEncryption { value } => ("unknown-encryption", Some(*value), None),
        EntryIssue::EncryptionBlockSize { size } => ("encryption-block-size", None, Some(*size)),
        EntryIssue::HashBlockSize { size } => ("hash-block-size", None, Some(*size)),
        EntryIssue::PathNotUtf8 => ("path-not-utf8", None, None),
        EntryIssue::Recovered => ("recovered-directory", None, None),
        EntryIssue::RecoveryLimit => ("recovery-limit", None, None),
        _ => ("unknown-directory-issue", None, None),
    };
    DirectoryIssue {
        entry_index: (!issue.is_archive_wide()).then_some(entry_index),
        code: format!("p4kx.archive.{code}"),
        archive_wide: issue.is_archive_wide(),
        detail: issue.to_string(),
        encryption_value,
        block_size,
    }
}
