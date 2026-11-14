// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_table::CatalogTable;
use crate::commodity_contract::CommodityPresentation;
use crate::contract_missions::ContractEntry;
use crate::mission_titles::MissionTitles;
use crate::quality_catalog::contract::QualityQuantizationResources;
use crate::schema_fields::every_field_required;

use super::catalog_item_components::resolution::WeaponMagazineResolution;
use super::catalog_weapon_projectile_ammo::WeaponProjectileAmmoSelection;

/// Explicit choices and derived values, separate from authored definitions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogProjections<'source> {
    pub(crate) contracts: CatalogTable<ContractEntry<'source>>,
    pub(crate) mission_titles: CatalogTable<MissionTitles<'source>>,
    pub(crate) blueprint_presentation: CatalogTable<
        crate::blueprint_presentation::BlueprintPresentation<
            p4kx_extraction::SourcedValue<crate::guid_text::CatalogGuid>,
        >,
    >,
    pub(crate) quality_quantization_resources: CatalogTable<QualityQuantizationResources>,
    pub(crate) tag_paths: CatalogTable<super::tag_catalog::TagPath>,
    pub(crate) commodity_presentation: CatalogTable<CommodityPresentation>,
    pub(crate) weapon_magazine_resolutions: CatalogTable<WeaponMagazineResolution>,
    pub(crate) weapon_projectile_ammo_selections: CatalogTable<WeaponProjectileAmmoSelection>,
    pub(crate) resource_group_parents:
        CatalogTable<super::resource_group_catalog::ResourceGroupParent>,
}
