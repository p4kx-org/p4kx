// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "tag_capture.rs"]
mod capture;
#[path = "tag_contract.rs"]
mod contract;
#[path = "tag_dependent_origins.rs"]
pub(crate) mod dependent_origins;
#[path = "tag_diagnostics.rs"]
mod diagnostics;
#[path = "entity_tag_read.rs"]
mod entities;
#[path = "tag_field_read.rs"]
mod fields;
#[path = "tag_header.rs"]
mod header;
#[path = "tag_path_capture.rs"]
mod path_capture;
#[path = "tag_path_row.rs"]
mod path_row;

pub(crate) use capture::{
    CapturedTagTree, CapturedTags, ContextualTagPath, OriginalTagPath, TagDatum, TagDependencies,
    TagNameSelector, TagPathLookup, TagPathMeaning, read,
};
pub(crate) use contract::{EntityTags, TagDefinition, TagRoots, TagRows};
pub(crate) use entities::read as entity;
pub(crate) use path_row::TagPath;
