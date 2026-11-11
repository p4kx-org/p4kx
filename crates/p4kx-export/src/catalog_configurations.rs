// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_entry::BrokerProjection;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_durability::{Degradation, Distortion, Durability};
use crate::schema_fields::every_field_required;
use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_projection::TemplateProjection;

/// Contextual configurations referring to their physical owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogConfigurations<'source> {
    pub(crate) blueprint_tiers: CatalogTable<crate::blueprint_tier_contract::BlueprintTier>,
    pub(crate) contract_templates: CatalogTable<TemplateProjection<'source>>,
    pub(crate) durability: CatalogTable<Durability>,
    pub(crate) degradation: CatalogTable<Degradation>,
    pub(crate) distortion: CatalogTable<Distortion>,
    pub(crate) mission_broker_entries: CatalogTable<BrokerProjection<'source>>,
    pub(crate) mission_flows: CatalogTable<MissionFlowConfiguration>,
    pub(crate) mission_properties: CatalogTable<MissionPropertyConfiguration>,
}
