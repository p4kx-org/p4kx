// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_entry::BrokerProjection;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_ammo_containers::AmmoContainer;
use crate::catalog_v3::catalog_durability::{Degradation, Distortion, Durability};
use crate::catalog_v3::catalog_explosives::Explosive;
use crate::catalog_v3::catalog_hacking_chips::HackingChip;
use crate::catalog_v3::catalog_quantum_drive::QuantumDrive;
use crate::catalog_v3::catalog_shield_generator::ShieldGenerator;
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::contract_recovery::{ItemRecoveryEconomy, ItemRecoveryOverrides};
use crate::recovery::ItemRecoveryPolicy;
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
    pub(crate) durability: CatalogTable<Durability<'source>>,
    pub(crate) degradation: CatalogTable<Degradation>,
    pub(crate) distortion: CatalogTable<Distortion>,
    pub(crate) explosives: CatalogTable<Explosive<'source>>,
    pub(crate) hacking_chips: CatalogTable<HackingChip<'source>>,
    pub(crate) ammo_containers: CatalogTable<AmmoContainer<'source>>,
    pub(crate) weapon_actions: CatalogTable<WeaponActions<'source>>,
    pub(crate) quantum_drives: CatalogTable<QuantumDrive<'source>>,
    pub(crate) shield_generators: CatalogTable<ShieldGenerator<'source>>,
    #[serde(flatten)]
    pub(crate) item_alignment:
        crate::catalog_v3::catalog_item_alignment::ItemAlignmentTables<'source>,
    pub(crate) mission_broker_entries: CatalogTable<BrokerProjection<'source>>,
    pub(crate) item_recovery_economy: CatalogTable<ItemRecoveryEconomy<'source>>,
    pub(crate) item_recovery_overrides: CatalogTable<ItemRecoveryOverrides>,
    pub(crate) item_recovery_policy: CatalogTable<ItemRecoveryPolicy<'source>>,
    pub(crate) mission_flows: CatalogTable<MissionFlowConfiguration>,
    pub(crate) mission_properties: CatalogTable<MissionPropertyConfiguration>,
}
