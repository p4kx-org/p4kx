// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source-state reads shared by canonical Catalog families.

#[path = "catalog_original.rs"]
pub(crate) mod original;

#[path = "catalog_fields.rs"]
mod catalog_fields;
#[path = "catalog_record_context.rs"]
mod catalog_record_context;

#[path = "catalog_reference_read.rs"]
mod catalog_reference_read;

#[path = "entry_json_object.rs"]
pub(crate) mod entry_json_object;
#[path = "loose_archive_path.rs"]
pub(crate) mod loose_archive_path;
#[path = "loose_json.rs"]
pub(crate) mod loose_json;
#[path = "parsed_entry_json.rs"]
pub(crate) mod parsed_entry_json;

pub(crate) use catalog_fields::CatalogFields;
pub(crate) use catalog_record_context::CatalogRecordContext;
pub(crate) use catalog_reference_read::reference_with_failure;
