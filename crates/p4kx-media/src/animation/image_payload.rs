// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::{Reference, SourceRange};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationImageIndex")]
pub(super) struct ImageIndex {
    pub(super) range: SourceRange,
    pub(super) layout: ImageLayout,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationImageLayout")]
pub(super) enum ImageLayout {
    CafRecords {
        header: SourceRange,
        record_count: usize,
        trailer: SourceRange,
    },
    DirectionalRecords {
        header: SourceRange,
        record_count: usize,
    },
    DirectionalOpaque,
    DirectionalPaths {
        declared_paths: usize,
    },
    UnsupportedVersion,
    UnsupportedFormat,
    InvalidFraming,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationImageRecord")]
pub(super) struct ImageRecord<'a> {
    pub(super) range: SourceRange,
    pub(super) path_field: SourceRange,
    pub(super) caf_path_crc32: u32,
    pub(super) dba_path_crc32: Option<u32>,
    #[schemars(schema_with = "crate::schema::optional_reference_definition")]
    pub(super) dba_entry: Option<Box<Reference>>,
    pub(super) dba_clip: DbaClipLookup<'a>,
    pub(super) path: ImagePath<'a>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationImagePath")]
pub(super) enum ImagePath<'a> {
    Valid {
        authored: &'a str,
        archive_path: Option<String>,
        #[schemars(schema_with = "crate::schema::reference_definition")]
        loose_entry: Box<Reference>,
    },
    InvalidEncoding,
    Unterminated,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationDirectionalPath")]
pub(super) struct ImageDirectionalPath<'a> {
    pub(super) path_field: SourceRange,
    pub(super) path: ImagePath<'a>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationImageDbaClipLookup")]
pub(super) enum DbaClipLookup<'a> {
    NoBankDeclared,
    BankUnresolved,
    BankUnreadable,
    InvalidCafPath,
    Observed {
        candidates_complete: bool,
        observed_candidates: usize,
        candidates: Vec<DbaClipCandidate<'a>>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationImageDbaClipCandidate")]
pub(super) struct DbaClipCandidate<'a> {
    pub(super) clip_ordinal: usize,
    pub(super) authored_name: &'a str,
    pub(super) name_range: SourceRange,
    #[serde(skip)]
    #[schemars(skip)]
    pub(super) proof: super::dba_metadata_lookup::MatchProof,
    pub(super) metadata_range: SourceRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationDirectionalRecord")]
pub(super) struct ImageDirectionalRecord {
    pub(super) range: SourceRange,
    pub(super) byte_length: u32,
    pub(super) header_word: u32,
    pub(super) path_field: SourceRange,
    pub(super) aim_path_crc32: u32,
    pub(super) opaque_tail: SourceRange,
}
