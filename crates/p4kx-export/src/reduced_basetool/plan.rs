// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;

pub(super) const LOCALES: [Language; 2] = [Language::English, Language::German];

pub(super) const FORMAT: &str = "p4kx-basetool-catalog";
pub(super) const FORMAT_VERSION: u32 = 1;
pub(super) const SELECTION: [&str; 76] = [
    "completeness",
    "configurations",
    "configurations.attachableModifiers",
    "configurations.blueprintTiers",
    "configurations.cargoLoadingGlobals",
    "configurations.clothing",
    "configurations.craftingGlobal",
    "configurations.durability",
    "configurations.facilities",
    "configurations.itemAttachmentConstraints",
    "configurations.itemPorts",
    "configurations.itemRadars",
    "configurations.jumpPointConfigurations",
    "configurations.missionBoards",
    "configurations.quantumDrives",
    "configurations.resourceComponents",
    "configurations.shieldGenerators",
    "configurations.shopCommodityGlobals",
    "configurations.suitArmor",
    "configurations.vehicleArmor",
    "configurations.weaponActions",
    "contractScenarios",
    "declarations",
    "declarations.blueprintRewardSources",
    "declarations.resourceGroupMembers",
    "declarations.shopInventoryResolution",
    "definitions",
    "definitions.ammoDefinitions",
    "definitions.blueprintCategories",
    "definitions.blueprintPools",
    "definitions.blueprints",
    "definitions.commodities",
    "definitions.contractScopes",
    "definitions.damageResistanceMacros",
    "definitions.dismantleProcesses",
    "definitions.factionReputations",
    "definitions.factions",
    "definitions.gameplayProperties",
    "definitions.items",
    "definitions.manufacturers",
    "definitions.reputationContexts",
    "definitions.reputationPerks",
    "definitions.reputationScopes",
    "definitions.reputationStandings",
    "definitions.resourceGroups",
    "definitions.resourceTypes",
    "definitions.shopInventories",
    "definitions.starmapAmenities",
    "definitions.starmapObjects",
    "definitions.starmapTypes",
    "exportHealth",
    "items",
    "loadouts",
    "paints",
    "placements",
    "placements.facilityInstances",
    "placements.jumpEndpoints",
    "placements.starmapPlacements",
    "projections",
    "projections.blueprintPoolWeights",
    "projections.blueprintPresentation",
    "projections.commodityPresentation",
    "projections.contracts",
    "projections.jumpConnections",
    "projections.missionTitles",
    "projections.resourceGroupParents",
    "projections.shipBaseFacts",
    "projections.weaponCombatBases",
    "projections.weaponProjectileAmmoSelections",
    "schemaVersion",
    "shipStorePledges",
    "shipStoreSlugs",
    "ships",
    "shops",
    "source",
    "texts",
];

#[cfg(test)]
pub(super) const TEXT_SCOPE: [&str; 2] = ["texts.publicNames", "texts.gameTexts"];
#[cfg(test)]
pub(super) const LANGUAGES: [&str; 2] = ["en", "de"];
pub(super) const COMPRESSED_LIMIT: u64 = 64_000_000;
pub(super) const DECODED_LIMIT: u64 = 1_000_000_000;
#[cfg(test)]
pub(super) const SHOP_SCOPE: [&str; 3] = [
    "declarations.shopInventoryResolution",
    "definitions.shopInventories",
    "shops",
];

#[cfg(test)]
pub(super) fn complete_scope(observed: &[&str]) -> bool {
    observed.len() == SELECTION.len() && SELECTION.iter().all(|path| observed.contains(path))
}
