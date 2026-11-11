// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, Filter, Kind};
use serde_json::{Value, json};

use super::{TestResult, catalog_with, class_names, options};

#[test]
fn every_kind_states_its_completeness() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(catalog["completeness"], expected_completeness(), "complete");
    let filter = Filter::new("*/spaceships/*");
    let (built, catalog) = catalog_with(&CatalogOptions {
        filter: Some(&filter),
        ..options()
    })?;
    assert_eq!(
        catalog["completeness"]["items"],
        json!({"status": "partial", "filter": "*/spaceships/*"}),
        "partial"
    );
    assert_eq!(class_names(&catalog, "items"), Vec::<&str>::new(), "none");
    assert_eq!(class_names(&catalog, "ships"), ["ACAS_Ship"], "the ship");
    assert_eq!(
        catalog["completeness"]["tags"],
        json!({"status": "complete"}),
        "the global Tag taxonomy retains its full selection scope"
    );
    assert!(
        built
            .summary()
            .kinds()
            .iter()
            .all(|entry| entry.complete == matches!(entry.kind, Kind::Tags | Kind::ShopInventories)),
        "the Tag taxonomy and physical shop inventory selection are global"
    );
    Ok(())
}

fn expected_completeness() -> Value {
    json!({
        "manufacturers": {"status": "complete"},
        "items": {"status": "complete"},
        "ships": {"status": "complete"},
        "commodities": {"status": "complete"},
        "blueprints": {"status": "complete"},
        "resourceGroups": {"status": "complete"},
        "resourceTypes": {"status": "complete"},
        "tags": {"status": "complete"},
        "blueprintCategories": {"status": "complete"},
        "blueprintCategoryLists": {"status": "complete"},
        "gameplayProperties": {"status": "complete"},
        "qualityDistributions": {"status": "complete"},
        "qualityQuantizations": {"status": "complete"},
        "blueprintPools": {"status": "complete"},
        "dismantleProcesses": {"status": "complete"},
        "paints": {"status": "complete"},
        "loadouts": {"status": "complete"},
        "starmapObjects": {"status": "complete"},
        "contracts": {"status": "complete"},
        "mineableElements": {"status": "complete"},
        "refiningProcesses": {"status": "complete"},
        "mineableCompositions": {"status": "complete"},
        "miningGlobalParams": {"status": "complete"},
        "harvestableProviderPresets": {"status": "complete"},
        "subHarvestableMultiConfigs": {"status": "complete"},
        "harvestableLocations": {"status": "complete"},
        "shops": {"status": "complete"},
        "shopInventories": {"status": "complete"},
        "explosionDefinitions": {"status": "complete"},
        "factionReputations": {"status": "complete"},
        "reputationScopes": {"status": "complete"},
        "reputationStandings": {"status": "complete"},
        "reputationContexts": {"status": "complete"},
        "missionTypes": {"status": "complete"},
        "missionLocalities": {"status": "complete"},
        "contractScenarios": {"status": "complete"},
        "cargoManifests": {"status": "complete"},
        "missionLocationTemplates": {"status": "complete"},
        "contractShipSpawns": {"status": "complete"},
        "contractLocationQueries": {"status": "complete"},
        "contractTemplates": {"status": "complete"},
        "missionBrokerEntries": {"status": "complete"},
        "contractNpcSpawns": {"status": "complete"},
        "contractEntitySpawns": {"status": "complete"},
        "missionPropertySets": {"status": "complete"},
        "lootTables": {"status": "complete"},
        "lootArchetypes": {"status": "complete"},
        "lootPoolFilters": {"status": "complete"},
        "legacyLootTables": {"status": "complete"},
        "legacyLootArchetypes": {"status": "complete"},
        "lootContainers": {"status": "complete"},
        "mineableEntities": {"status": "complete"},
        "crafters": {"status": "complete"},
        "shipStoreSlugs": {"status": "complete"},
    })
}
