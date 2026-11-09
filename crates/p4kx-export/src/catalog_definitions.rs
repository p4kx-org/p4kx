// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_category::BlueprintCategoryDefinition;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::commodity_contract::CommodityDefinition;
use crate::manufacturer::ManufacturerDefinition;
use crate::quality_catalog::contract::{
    QualityDistributionDefinition, QualityQuantizationDefinition,
};
use crate::resource_type_contract::ResourceType;
use crate::schema_fields::every_field_required;

/// Canonical typed definitions with distinct physical identities and source scopes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDefinitions<'source> {
    pub(crate) blueprint_categories: CatalogTable<BlueprintCategoryDefinition<'source>>,
    pub(crate) gameplay_properties: CatalogTable<GameplayPropertyDefinition<'source>>,
    pub(crate) quality_distributions: CatalogTable<QualityDistributionDefinition<'source>>,
    pub(crate) quality_quantizations: CatalogTable<QualityQuantizationDefinition<'source>>,
    pub(crate) tags: CatalogTable<super::tag_catalog::TagDefinition<'source>>,
    pub(crate) resource_groups:
        CatalogTable<super::resource_group_catalog::ResourceGroupDefinition<'source>>,
    pub(crate) resource_types: CatalogTable<ResourceType<'source>>,
    pub(crate) commodities: CatalogTable<CommodityDefinition<'source>>,
    pub(crate) manufacturers: CatalogTable<ManufacturerDefinition<'source>>,
}
