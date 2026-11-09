// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "tag_capture.rs"]
mod capture;
#[path = "tag_contract.rs"]
mod contract;
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

pub(crate) use capture::read;
pub(crate) use contract::{EntityTags, TagDefinition, TagPath, TagRoots, TagRows};
pub(crate) use entities::read as entity;
