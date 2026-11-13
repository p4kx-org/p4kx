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
