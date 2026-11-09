// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_table::CatalogTable;
use crate::commodity_contract::CommodityPresentation;
use crate::schema_fields::every_field_required;

/// Explicit choices and derived values, separate from authored definitions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogProjections {
    pub(crate) tag_paths: CatalogTable<super::tag_catalog::TagPath>,
    pub(crate) commodity_presentation: CatalogTable<CommodityPresentation>,
    pub(crate) resource_group_parents:
        CatalogTable<super::resource_group_catalog::ResourceGroupParent>,
}
