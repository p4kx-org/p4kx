// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source-state reads shared by canonical Catalog families.

#[path = "catalog_fields.rs"]
mod catalog_fields;
#[path = "catalog_record_context.rs"]
mod catalog_record_context;

pub(crate) use catalog_fields::CatalogFields;
pub(crate) use catalog_record_context::CatalogRecordContext;
