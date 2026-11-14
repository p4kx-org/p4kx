// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::{ClipDatabaseUnavailable, ReadStatus};
use crate::SourceRange;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityNamedDbaBank")]
pub(in super::super) struct Bank<'a> {
    pub(in super::super) archive_path: &'a str,
    pub(in super::super) database_parameter: usize,
    pub(in super::super) source: Option<usize>,
    pub(in super::super) state: BankState,
    pub(in super::super) observed_matches: usize,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationEntityNamedDbaMatch")]
pub(in super::super) struct BankMatch<'a> {
    pub(in super::super) database_parameter: usize,
    pub(in super::super) source: usize,
    pub(in super::super) clip_ordinal: usize,
    pub(in super::super) authored_name: &'a str,
    pub(in super::super) name_range: SourceRange,
    pub(in super::super) metadata_range: SourceRange,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationEntityNamedDbaState")]
pub(in super::super) enum BankState {
    Enumerated,
    Incomplete,
    InvalidPath,
    Missing,
    Ambiguous,
    NotSelected,
    KeyRequired,
    WrongKey,
    Unavailable,
    Malformed,
}

impl BankState {
    pub(in super::super) const fn from_lookup(reason: &ClipDatabaseUnavailable) -> Self {
        match reason {
            ClipDatabaseUnavailable::UnsupportedPath => Self::InvalidPath,
            ClipDatabaseUnavailable::Missing => Self::Missing,
            ClipDatabaseUnavailable::Ambiguous => Self::Ambiguous,
            _ => Self::Unavailable,
        }
    }

    pub(in super::super) const fn from_read(status: ReadStatus) -> Self {
        match status {
            ReadStatus::NotSelected => Self::NotSelected,
            ReadStatus::KeyRequired => Self::KeyRequired,
            ReadStatus::WrongKey => Self::WrongKey,
            ReadStatus::Malformed => Self::Malformed,
            _ => Self::Unavailable,
        }
    }
}
