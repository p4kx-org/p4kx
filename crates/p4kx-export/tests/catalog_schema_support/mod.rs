// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A minimal catalog that holds one entity of every kind and every field the schema lists.

use serde_json::json;

mod contracts;
mod crafters;
mod harvest;
mod loot;
mod recovery;
mod reputation;
mod slugs;
mod spawns;

use recovery::item_recovery;

pub(crate) fn minimal_catalog() -> serde_json::Value {
    json!({
        "schemaVersion": 2,
        "source": {
            "channel": null,
            "build": null,
            "archiveSize": 0,
            "archiveModified": null,
            "toolVersion": "0.1.0",
            "exportedAt": "2026-10-18T00:00:00Z"
        },
        "completeness": {
            "manufacturers": {"status": "complete"},
            "items": {"status": "complete"},
            "ships": {"status": "partial", "filter": "*/spaceships/*"},
            "commodities": {"status": "complete"},
            "blueprints": {"status": "complete"},
            "resourceGroups": {"status": "complete"},
            "blueprintCategories": {"status": "partial", "filter": "*/blueprintcategories/*"},
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
            "factionReputations": {"status": "complete"},
            "reputationScopes": {"status": "complete"},
            "reputationStandings": {"status": "complete"},
            "reputationContexts": {"status": "complete"},
            "missionTypes": {"status": "complete"},
            "missionLocalities": {"status": "complete"},
            "contractScenarios": {"status": "complete"},
            "missionLocationTemplates": {"status": "complete"},
            "contractShipSpawns": {"status": "complete"},
            "contractLocationQueries": {"status": "complete"},
            "contractTemplates": {"status": "complete"},
            "lootTables": {"status": "complete"},
            "lootArchetypes": {"status": "complete"},
            "lootPoolFilters": {"status": "complete"},
            "legacyLootTables": {"status": "complete"},
            "legacyLootArchetypes": {"status": "complete"},
            "lootContainers": {"status": "complete"},
            "mineableEntities": {"status": "complete"},
            "crafters": {"status": "complete"},
            "cargoManifests": {"status": "complete"},
            "shipStoreSlugs": {"status": "complete"}
        },
        "manufacturers": [{
            "guid": "cfc0122d-5275-415a-a656-3dcb3346feb5",
            "className": "ACAS",
            "recordPath": "libs/foundry/records/scitemmanufacturer/acas",
            "code": "ACAS",
            "name": {"key": "@manufacturer_NameACAS", "en": "Ace Astrogation", "de": null},
            "description": {"key": null, "en": null, "de": null}
        }],
        "items": [{
            "guid": "30000000-0000-4000-8000-000000000001",
            "className": "POWR_ACAS_S01",
            "recordPath": "libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01",
            "name": {"key": "@item_NamePOWR_ACAS_S01", "en": "Lumen", "de": null},
            "description": {"key": null, "en": null, "de": null},
            "type": "PowerPlant",
            "subType": "Power",
            "size": 1,
            "grade": 3,
            "mass": 125.5,
            "manufacturerGuid": "cfc0122d-5275-415a-a656-3dcb3346feb5",
            "tags": ["flightReady"],
            "displayType": {"key": null, "en": null, "de": null},
            "itemClass": "Military",
            "playability": "player",
            "stats": null,
            "variantOf": null
        }],
        "ships": [{
            "guid": "90000000-0000-4000-8000-000000000001",
            "className": "ACAS_Ship",
            "recordPath": "libs/foundry/records/entities/spaceships/acas_ship",
            "name": {"key": null, "en": null, "de": null},
            "description": {"key": null, "en": null, "de": null},
            "role": {"key": "@vehicle_class_lightfreight", "en": "Light Freight", "de": null},
            "career": {"key": null, "en": null, "de": null},
            "crewSize": 2,
            "manufacturerGuid": null,
            "vehicleDefinition": null,
            "vehicleKind": "spaceship",
            "size": 3,
            "mass": 1210.0,
            "massBasis": "parts",
            "hullHealth": 600.0,
            "cargoScu": 18.5,
            "cargoGridsWithIssue": 0,
            "playability": "ai",
            "variantOf": "a0000000-0000-4000-8000-000000000001",
            "insurance": {
                "restoreOnClaim": true,
                "shipEntityClassName": "AEGS_Avenger_Titan",
                "baseWaitTimeMinutes": 8.5,
                "mandatoryWaitTimeMinutes": 2.5,
                "expediteFee": 3200
            }
        }],
        "commodities": [{
            "guid": "a1000000-0000-4000-8000-000000000001",
            "className": "gold",
            "recordPath": "libs/foundry/records/entities/commodities/metals/gold",
            "name": {"key": "@items_commodities_gold", "en": "Gold", "de": null},
            "description": {"key": null, "en": null, "de": null},
            "typeGuid": "b1000000-0000-4000-8000-000000000001",
            "subtypeGuid": null,
            "displayType": {"key": null, "en": null, "de": null},
            "densityGramsPerCubicCentimetre": null,
            "refinedSubtypeGuid": null
        }],
        "blueprints": [{
            "guid": "11000000-0000-4000-8000-000000000001",
            "className": "BP_CRAFT_core",
            "recordPath": "libs/foundry/records/crafting/blueprints/crafting/bp_craft_core",
            "key": "BP_CRAFT_core",
            "name": {"key": null, "en": null, "de": null},
            "categoryGuid": "d1000000-0000-4000-8000-000000000001",
            "producedItemGuid": null,
            "craftTimeSeconds": 93784,
            "ingredients": [
                {
                    "resourceGuid": "b1000000-0000-4000-8000-000000000001",
                    "itemGuid": null,
                    "quantityScu": 0.5,
                    "quantityUnits": null,
                    "minQuality": 0,
                    "slot": "CORE",
                    "slotKey": null
                },
                {
                    "resourceGuid": null,
                    "itemGuid": "30000000-0000-4000-8000-000000000001",
                    "quantityScu": null,
                    "quantityUnits": 5,
                    "minQuality": 500,
                    "slot": null,
                    "slotKey": "@crafting_ui_slotname_core"
                }
            ],
            "tiers": tiers(),
            "defaultAvailable": false,
            "dismantle": {
                "processGuid": "f1000000-0000-4000-8000-000000000001",
                "tierIndex": 0
            }
        }],
        "resourceGroups": [{
            "guid": "b1000000-0000-4000-8000-000000000001",
            "className": "Metal",
            "recordPath": "libs/foundry/records/resourcetypedatabase/resourcetypedatabase",
            "name": {"key": "@items_commodities_type_metal", "en": "Metal", "de": null},
            "parentGuid": null
        }],
        "blueprintCategories": [{
            "guid": "d1000000-0000-4000-8000-000000000001",
            "className": "Ammo",
            "recordPath": "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase",
            "name": {"key": null, "en": null, "de": null}
        }],
        "gameplayProperties": [{
            "guid": "e1000000-0000-4000-8000-000000000001",
            "className": "GPP_Armor",
            "recordPath": "libs/foundry/records/crafting/craftedproperties/gpp_armor",
            "name": {"key": "@StatName_GPP_Armor", "en": "Armour", "de": null}
        }],
        "qualityDistributions": [{
            "guid": "e2000000-0000-4000-8000-000000000001",
            "className": "Creature_Pyro",
            "recordPath": "libs/foundry/records/crafting/qualitydistribution/creature_pyro",
            "distribution": null,
            "overrides": [{
                "locationGuid": "e3000000-0000-4000-8000-000000000001",
                "distribution": {
                    "kind": "CraftingQualityDistributionNormal",
                    "min": 1.0, "max": 1000.0, "mean": 269.0, "stddev": null
                }
            }]
        }],
        "qualityQuantizations": [{
            "guid": "e4000000-0000-4000-8000-000000000001",
            "className": "quantization_gold",
            "recordPath": "libs/foundry/records/crafting/qualityquantization/quantization_gold",
            "bands": [
                {"start": 0, "end": 399, "mappedValue": 264},
                {"start": 400, "end": 1000, "mappedValue": 553}
            ],
            "resourceTypeGuids": ["b1000000-0000-4000-8000-000000000001"]
        }],
        "blueprintPools": pools(),
        "dismantleProcesses": [{
            "guid": "f1000000-0000-4000-8000-000000000001",
            "className": "GlobalGenericDismantle",
            "recordPath": "libs/foundry/records/crafting/blueprints/dismantle/globalgenericdismantle",
            "efficiency": 0.5,
            "timeSeconds": 15
        }],
        "craftingGlobal": {
            "dismantleExcludedResourceGuids": ["b1000000-0000-4000-8000-000000000001"],
            "dismantleExcludedItemGuids": []
        },
        "paints": [{
            "itemGuid": "30000000-0000-4000-8000-000000000001",
            "shipGuids": ["90000000-0000-4000-8000-000000000001"]
        }],
        "loadouts": loadouts(),
        "starmapObjects": [{
            "guid": "a2000000-0000-4000-8000-000000000001",
            "className": "Stanton1_Hurston",
            "recordPath": "libs/foundry/records/starmap/pu/system/stanton/stanton1_hurston",
            "name": {"key": "@Stanton1_Name", "en": "Hurston", "de": null},
            "type": "Planet",
            "typeGuid": "a2000000-0000-4000-8000-000000000002",
            "parentGuid": "a2000000-0000-4000-8000-000000000003",
            "hideInStarmap": false,
            "system": "stanton",
            "entityName": "planet_hurston",
            "placedUnderGuid": "a2000000-0000-4000-8000-000000000003",
            "position": [12_850_000_000.0, 1250.5, -3.0]
        }],
        "contracts": contracts::contracts(),
        "mineableElements": [{
            "guid": "a3000000-0000-4000-8000-000000000001",
            "className": "agricium_ore",
            "recordPath": "libs/foundry/records/mining/mineableelements/agricium_ore",
            "resourceTypeGuid": "b1000000-0000-4000-8000-000000000001",
            "instability": 350.0,
            "resistance": 0.5,
            "optimalWindowMidpoint": 0.5,
            "optimalWindowMidpointRandomness": 0.15,
            "optimalWindowThinness": 2.0,
            "explosionMultiplier": 4.0,
            "clusterFactor": 0.2
        }],
        "refiningProcesses": [{
            "guid": "a4000000-0000-4000-8000-000000000001",
            "className": "processingtype-fastcareful",
            "recordPath": "libs/foundry/records/refiningprocess/processingtype-fastcareful",
            "processName": {
                "key": "@refinery_ui_ProcessingType_FastCareful",
                "en": null,
                "de": null
            },
            "refiningSpeed": "Fast",
            "refiningQuality": "Careful"
        }],
        "mineableCompositions": harvest::compositions(),
        "miningGlobalParams": [harvest::mining_global_params()],
        "harvestableProviderPresets": harvest::presets(),
        "subHarvestableMultiConfigs": harvest::multi_configs(),
        "harvestableLocations": harvest::locations(),
        "shops": harvest::shops(),
        "factionReputations": reputation::factions(),
        "reputationScopes": reputation::scopes(),
        "reputationStandings": reputation::standings(),
        "reputationContexts": reputation::contexts(),
        "missionTypes": reputation::mission_types(),
        "missionLocalities": reputation::localities(),
        "contractScenarios": reputation::scenarios(),
        "missionLocationTemplates": spawns::location_templates(),
        "contractShipSpawns": spawns::ship_spawns(),
        "contractLocationQueries": spawns::location_queries(),
        "contractTemplates": spawns::contract_templates(),
        "lootTables": loot::tables(),
        "lootArchetypes": loot::archetypes(),
        "lootPoolFilters": loot::filters(),
        "legacyLootTables": loot::legacy_tables(),
        "legacyLootArchetypes": loot::legacy_archetypes(),
        "lootContainers": loot::containers(),
        "mineableEntities": harvest::mineable_entities(),
        "crafters": crafters::crafters(),
        "cargoManifests": contracts::manifests(),
        "shipStoreSlugs": slugs::ship_store_slugs(),
        "itemRecovery": item_recovery()
    })
}

fn loadouts() -> serde_json::Value {
    let gun = json!({
        "name": "hardpoint_gun_left", "source": "loadout",
        "minSize": 1, "maxSize": 3,
        "types": [{"type": "WeaponGun", "subTypes": ["Gun"]}],
        "flags": ["uneditable"], "portTags": ["Gun"],
        "requiredTags": ["Mount"],
        "displayNameKey": null, "visibility": "visible",
        "itemGuid": "30000000-0000-4000-8000-000000000001",
        "itemClassName": "POWR_ACAS_S01", "dockedVehicle": false, "unresolved": null,
        "ports": []
    });
    json!([{
        "entityGuid": "90000000-0000-4000-8000-000000000001",
        "entityKind": "ship",
        "issues": ["vehicle_implementation"],
        "ports": [{
            "name": "hardpoint_turret", "source": "vehicle",
            "minSize": null, "maxSize": null,
            "types": [], "flags": [], "portTags": [],
            "requiredTags": [],
            "displayNameKey": null, "visibility": "unspecified",
            "itemGuid": null, "itemClassName": null, "dockedVehicle": false,
            "unresolved": null,
            "ports": [gun]
        }]
    }])
}

fn pools() -> serde_json::Value {
    json!([{
        "guid": "e4000000-0000-4000-8000-000000000001",
        "className": "BP_REWARDS_Haul",
        "recordPath": "libs/foundry/records/crafting/blueprintrewards/bp_rewards_haul",
        "totalWeight": 4.0,
        "entries": [{
            "blueprintGuid": "11000000-0000-4000-8000-000000000001",
            "weight": 1.0
        }],
        "rewardSources": [{
            "recordGuid": "e5000000-0000-4000-8000-000000000001",
            "recordPath": "libs/foundry/records/contracts/contractgenerator/haul",
            "contractId": null, "contractName": "Haul_Intro", "chance": 0.5
        }]
    }])
}

fn tiers() -> serde_json::Value {
    let line = json!({
        "kind": "resource", "count": null, "slot": null, "slotKey": null,
        "options": [],
        "resourceGuid": "b1000000-0000-4000-8000-000000000001",
        "itemGuid": null, "quantityScu": 0.5, "quantityUnits": null,
        "minQuality": 0, "quantityMultiplier": 1.15,
        "compositionInclusion": "Include",
        "modifiers": [{
            "propertyGuid": "e1000000-0000-4000-8000-000000000001",
            "ranges": [{
                "kind": "multiplier", "startQuality": 0, "endQuality": 1000,
                "atStart": 0.85, "atEnd": 1.15
            }]
        }]
    });
    json!([{
        "craftTimeSeconds": 93784,
        "mandatory": {
            "kind": "select", "count": 1, "slot": "CORE", "slotKey": null,
            "options": [line],
            "resourceGuid": null, "itemGuid": null, "quantityScu": null,
            "quantityUnits": null, "minQuality": null, "quantityMultiplier": null,
            "compositionInclusion": null, "modifiers": []
        },
        "optional": []
    }])
}
