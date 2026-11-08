// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_table::CatalogTable;
use crate::commodity_contract::CommodityDefinition;
use crate::resource_type_contract::ResourceType;
use crate::schema_fields::every_field_required;

/// Canonical typed definitions with distinct physical identities and source scopes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDefinitions<'source> {
    pub(crate) resource_types: CatalogTable<ResourceType<'source>>,
    pub(crate) commodities: CatalogTable<CommodityDefinition<'source>>,
}
