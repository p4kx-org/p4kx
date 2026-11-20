// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod assemble;
mod commodity_dependencies;
mod fault;
mod mode;
mod project;
mod projection;
mod quality_dependencies;
mod read;
mod resource_scope;
mod selection;

use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyRows;
use crate::export_ledger::ExportLedger;

use super::Crafting;

pub(crate) use commodity_dependencies::CommodityDependencies;
pub(crate) use mode::ReadMode;
pub(crate) use quality_dependencies::QualityDependencies;
pub(crate) use read::{read, read_catalog};
pub(crate) use selection::{gameplay_property, pool_record};

/// The public crafting result and actual production-source observations.
pub(crate) struct ReadCrafting<'f> {
    pub(crate) data: Crafting<'f>,
    pub(crate) blueprints: Vec<crate::blueprint_catalog_read::PreparedBlueprint<'f>>,
    pub(crate) blueprint_rules: crate::blueprint_rule_inputs::BlueprintRules,
    pub(crate) globals: Vec<crate::catalog_v3::crafting_global::PreparedCraftingGlobal<'f>>,
    pub(crate) pools: Vec<
        crate::catalog_v3::industry_modules::blueprint_pool_definition::PreparedBlueprintPool<'f>,
    >,
    pub(crate) reward_declarations: Vec<
        crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'f>,
    >,
    pub(crate) reward_numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
    pub(crate) catalog_failures: Vec<crate::catalog_read_failure::CatalogReadFailure<'f>>,
    pub(crate) ledger: ExportLedger,
    pub(crate) gameplay: GameplayPropertyRows<'f>,
    pub(crate) quality: crate::quality_catalog::QualityRows<'f>,
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_tests;
