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

pub(crate) use catalog_fields::CatalogFields;
pub(crate) use catalog_record_context::CatalogRecordContext;
pub(crate) use catalog_reference_read::reference_with_failure;
