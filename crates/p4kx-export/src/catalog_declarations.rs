// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_table::CatalogTable;
use super::tag_catalog::{EntityTags, TagRoots};
use crate::schema_fields::every_field_required;

/// Original ordered uses, independently of reusable definitions and derived paths.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDeclarations<'source> {
    pub(crate) tag_roots: CatalogTable<TagRoots<'source>>,
    pub(crate) entity_tags: CatalogTable<EntityTags<'source>>,
    pub(crate) resource_group_members:
        CatalogTable<super::resource_group_catalog::ResourceGroupMembers>,
}
