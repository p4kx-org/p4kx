// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic catalog for structural schema checks.

use p4kx_export::Kind;
use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

mod blueprint_categories;
mod blueprint_category_lists;
mod blueprints;
mod commodities;
mod contract_properties;
mod contracts;
mod crafters;
mod dataset_availability;
mod gameplay_properties;
mod harvest;
mod health;
mod loadouts;
mod loot;
mod mining;
mod mission_sets;
mod npc_spawns;
mod public_fixtures;
mod quality;
mod recovery;
mod reputation;
mod resource_groups;
mod resources;
mod ships;
mod slugs;
mod source_values;
mod spawns;
mod tags;

use self::public_fixtures::{manufacturer, public_names};

fn selection_completeness() -> Value {
    let mut kinds: serde_json::Map<String, Value> = Kind::ALL
        .into_iter()
        .map(|kind| (kind.name().to_owned(), json!({"status": "complete"})))
        .collect();
    for (kind, filter) in [
        (Kind::Ships, "*/spaceships/*"),
        (Kind::BlueprintCategories, "*/blueprintcategories/*"),
    ] {
        kinds.insert(
            kind.name().to_owned(),
            json!({"status": "partial", "filter": filter}),
        );
    }
    kinds.insert("shipStoreSlugs".into(), json!({"status": "complete"}));
    Value::Object(kinds)
}

#[expect(
    clippy::too_many_lines,
    reason = "the synthetic JSON specimen covers the full catalog structure"
)]
pub(crate) fn minimal_catalog() -> Value {
    let mut catalog = json!({
        "schemaVersion": p4kx_export::SCHEMA_VERSION,
        "source": {
            "channel": null,
            "build": null,
            "archiveSize": 0,
            "archiveModified": null,
            "archiveKey": "given",
            "encryptedEntriesUnread": 0,
            "toolVersion": "0.1.0",
            "exportedAt": "2026-10-18T00:00:00Z"
        },
        "completeness": selection_completeness(),
        "items": [{
            "recordIndex": source_values::present(json!(1)),
            "guid": source_values::present(json!("30000000-0000-4000-8000-000000000001")),
            "recordName": source_values::present(json!("EntityClassDefinition.POWR_ACAS_S01")),
            "className": source_values::present(json!("POWR_ACAS_S01")),
            "fileName": source_values::present(json!("libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01.xml")),
            "recordPath": source_values::present(json!("libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01")),
            "sourceType": source_values::present(json!("EntityClassDefinition")),
            "metadataTag": source_values::source(SourceValue::Absent),
            "selection": {
                "attachmentComponentOrdinal": source_values::present(json!(0)),
                "physicsComponentOrdinal": source_values::present(json!(1)),
                "purchasableComponentOrdinal": source_values::present(json!(2))
            },
            "itemClass": source_values::present(json!("Military")),
            "playability": source_values::present(json!("player")),
            "variantOf": null
        }],
        "ships": ships::rows(),
        "definitions": {"items": {"availability": "available", "rows": []}, "contractGenerators": absent_table(), "missionScenarios": absent_table(), "blueprints": blueprints::definitions(), "blueprintPools": blueprints::pools(), "contractTemplates": absent_table(), "missionBrokerEntries": absent_table(), "lootPoolFilters": loot::filters(), "blueprintCategories": blueprint_categories::definitions(), "gameplayProperties": gameplay_properties::definitions(), "qualityDistributions": quality::distributions(), "qualityQuantizations": quality::quantizations(), "resourceTypes": resources::table(), "resourceGroups": resource_groups::definitions(),
            "commodities": commodities::definitions(), "manufacturers": manufacturer(),
            "dismantleProcesses": blueprints::dismantle_processes(),
            "missionTypes": reputation::mission_types(),
            "missionLocalities": reputation::localities(),
            "refiningProcesses": mining::refining(),
            "mineableEntities": harvest::mineable_entities(),
            "tags": tags::definitions(),
            "explosionDefinitions": absent_table(), "damageResistanceMacros": absent_table(), "radarSystemSharedParams": absent_table(), "turretHealthModifiers": absent_table(), "miningLaserGlobals": absent_table(), "consumableTypes": absent_table(), "consumableSubtypes": absent_table(),
            "inventoryContainers": absent_table(),
            "medicalItemTierConfigs": absent_table(),
            "factions": absent_table(),
            "factionReputations": reputation::factions(),
            "reputationPerks": absent_table(),
            "reputationTriggerRecords": absent_table(),
            "reputationRewardAmounts": absent_table(),
            "infractions": absent_table(),
            "infractionSets": absent_table(),
            "jurisdictions": absent_table(),
            "locationClasses": absent_table(),
            "npcArchetypes": absent_table(),
            "npcClassPools": absent_table(),
            "autoSpawns": absent_table(),
            "autoSpawnSets": absent_table(),
            "encounterIndices": absent_table(),
            "locationFunctions": absent_table(),
            "locationSizes": absent_table(),
            "locationRegistries": {
                "classes": absent_table(),
                "functions": absent_table(),
                "sizes": absent_table(),
            },
            "starmapObjects": absent_table(),
            "starmapTypes": absent_table(),
            "starmapPresets": absent_table(),
            "starmapAmenities": absent_table(),
            "ammoDefinitions": absent_table(),
            "cargoManifests": contracts::manifests(),
            "weaponAnimationSources": {
                "player": null,
                "actorBindings": [],
                "controllers": [],
                "databases": [],
                "tagDefinitions": [],
                "scopes": [],
                "branches": [],
                "fragmentSelectors": []
            },
            "weaponActionRecoilDefinitions": absent_table(),
            "weaponProceduralAnimations": absent_table(),
            "actorProceduralRecoilConfigs": absent_table(),
            "weaponProceduralClips": absent_table(),
            "actorProceduralRecoilModifiers": absent_table(),
            "reputationContexts": reputation::contexts(),
            "reputationScopes": reputation::scopes(),
            "reputationStandings": reputation::standings(),
            "shopInventories": {"availability": "absent", "meaning": "offlineDesignTime", "rows": []}},
        "configurations": {
            "miningGlobal": mining::globals(),
            "craftingGlobal": blueprints::globals(),
            "blueprintTiers": blueprints::tiers(),
            "itemRecoveryEconomy": recovery::economy(),
            "itemRecoveryOverrides": recovery::overrides(),
            "itemRecoveryPolicy": recovery::policy(),
            "durability": absent_table(),
            "degradation": absent_table(),
            "distortion": absent_table(),
            "emp": absent_table(),
            "explosives": absent_table(),
            "hackingChips": absent_table(),
            "ammoContainers": absent_table(),
            "weaponActions": absent_table(),
            "missileConfigurations": absent_table(),
            "jumpDrives": absent_table(),
            "jumpDriveFlightTuning": absent_table(),
            "jumpTunnelForces": absent_table(),
            "quantumDrives": absent_table(),
            "quantumInterdiction": absent_table(),
            "shieldGenerators": absent_table(),
            "itemPorts": absent_table(),
            "itemAttachmentConstraints": absent_table(),
            "fuelTanks": absent_table(),
            "fuelIntakes": absent_table(),
            "resourceContainers": absent_table(),
            "facilities": absent_table(),
            "inventoryConfigurations": absent_table(),
            "itemBounds": absent_table(),
            "interactions": absent_table(),
            "resourceComponents": absent_table(),
            "resourceConnections": absent_table(),
            "resourceNetworkParameters": absent_table(),
            "missionBoards": absent_table(),
            "entityAnimationControllers": absent_table(),
            "lootGeneration": absent_table(),
            "hackableDevices": absent_table(), "jumpPointConfigurations": absent_table(), "globalJumpPointConfigurations": absent_table(), "countermeasureEmitters": absent_table(),
            "vehicleArmor": absent_table(), "suitArmor": absent_table(), "clothing": absent_table(), "itemRadars": absent_table(), "turretHealth": absent_table(), "miningLasers": absent_table(), "attachableModifiers": absent_table(), "consumables": absent_table(),
            "contractTemplates": absent_table(),
            "missionBrokerEntries": absent_table(),
            "clientProfileSettings": {
                "documents": [
                    {"kind": "general", "requestedPath": "Data/fixture/attributes.xml",
                        "status": {"state": "present", "value": "missing", "sourceUsesRefs": [],
                            "sourceSetComplete": false, "containsEncryptedSource": null},
                        "candidates": [], "elements": []},
                    {"kind": "virtualReality", "requestedPath": "Data/fixture/attributesVR.xml",
                        "status": {"state": "present", "value": "missing", "sourceUsesRefs": [],
                            "sourceSetComplete": false, "containsEncryptedSource": null},
                        "candidates": [], "elements": []}
                ]
            },
            "localContractGenerators": absent_table(),
            "missionFlows": absent_table(),
            "reputationTriggers": absent_table(),
            "missionProperties": absent_table()
        },
        "declarations": {
            "blueprintCategoryLists": blueprint_category_lists::declarations(),
            "blueprintRewardSources": blueprints::reward_sources(),
            "tagRoots": tags::roots(),
            "shopInventoryResolution": absent_table(),
            "entityTags": tags::entities(),
            "lootEntityContainerDocuments": absent_table(),
            "ammoBindings": absent_table(),
            "contractScenarioRequirements": absent_table(),
            "resourceNetworkLinks": absent_table(),
            "roomAssociations": absent_table(),
            "itemRoomAssociations": absent_table(),
            "resourceGroupMembers": resource_groups::members()
        },
        "placements": {
            "installedConfigurations": absent_table(), "jumpEndpoints": absent_table(),
            "locationInstances": absent_table(),
            "resourceNetworkNodes": absent_table(),
            "resourceNetworkLinkBindings": absent_table(),
            "roomAssociationBindings": absent_table(),
            "starmapPlacements": absent_table(),
            "facilityInstances": absent_table(),
            "facilityInteriorCoverage": [],
            "containerInstances": absent_table(),
            "locationPhysicsGrids": absent_table(),
            "unresolvedContainerLinks": absent_table()
        },
        "texts": {
            "languagePolicy": {"selection": "availableSupported", "fallback": "none"},
            "languageSources": source_values::language_sources(),
            "publicNames": public_names(),
            "gameTexts": [reputation::mission_type_text(), spawns::location_name_text()]
        },
        "projections": {"contracts": contracts::contracts(), "missionTitles": contracts::titles(), "blueprintPresentation": blueprints::presentation(), "blueprintPoolWeights": blueprints::pool_weights(), "qualityQuantizationResources": quality::inverse(), "commodityPresentation": commodities::presentation(), "tagPaths": tags::paths(), "resourceGroupParents": resource_groups::parents(), "weaponMagazineResolutions": absent_table(), "weaponProjectileAmmoSelections": absent_table(), "weaponPlayerAnimationCandidates": absent_table(), "weaponCombatBases": absent_table(), "jumpConnections": absent_table()},
        "provenance": {"sources": [], "uses": []},

        "paints": [{
            "itemGuid": "30000000-0000-4000-8000-000000000001",
            "shipGuids": ["90000000-0000-4000-8000-000000000001"]
        }],
        "loadouts": loadouts::loadouts(),
        "mineableElements": [mineable_element()],
        "mineableCompositions": harvest::compositions(),
        "harvestableProviderPresets": harvest::presets(),
        "subHarvestableMultiConfigs": harvest::multi_configs(),
        "harvestableLocations": harvest::locations(),
        "shops": harvest::shops(),
        "contractScenarios": reputation::scenarios(),
        "missionLocationTemplates": spawns::location_templates(),
        "contractShipSpawns": spawns::ship_spawns(),
        "contractLocationQueries": spawns::location_queries(),
        "missionPropertySets": mission_sets::sets(),
        "contractNpcSpawns": npc_spawns::npc_spawns(),
        "contractEntitySpawns": npc_spawns::entity_spawns(),
        "lootTables": loot::tables(),
        "lootArchetypes": loot::archetypes(),
        "legacyLootTables": loot::legacy_tables(),
        "legacyLootArchetypes": loot::legacy_archetypes(),
        "lootContainers": loot::containers(),
        "crafters": crafters::crafters(),
        "shipStoreSlugs": slugs::ship_store_slugs()
    });
    dataset_availability::add_defaults(&mut catalog);
    loot::add_definition_tables(&mut catalog);
    let export_health = health::fixture(&catalog);
    let Value::Object(fields) = &mut catalog else {
        unreachable!("the authored catalog fixture is a JSON object");
    };
    fields.insert("exportHealth".to_owned(), export_health);
    catalog
}

fn absent_table() -> Value {
    json!({"availability": "absent", "rows": []})
}

fn mineable_element() -> Value {
    let present = source_values::present;
    let path = "libs/foundry/records/mining/mineableelements/agricium_ore";
    let single = |finite: f32| {
        present(json!({
            "rawBits": format!("{:08x}", finite.to_bits()),
            "finite": finite,
        }))
    };
    json!({
        "guid": present(json!("a3000000-0000-4000-8000-000000000001")),
        "recordName": present(json!("MineableElement.agricium_ore")),
        "className": present(json!("agricium_ore")),
        "recordPath": present(json!(path)),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("MineableElement")),
        "fileName": present(json!(format!("{path}.xml"))),
        "metadataTag": source_values::source(SourceValue::Absent),
        "resourceType": present(json!({
            "targetRecordIndex": present(json!(1)),
            "guid": present(json!("b1000000-0000-4000-8000-000000000001")),
            "recordPath": present(json!("libs/foundry/records/resourcetypedatabase/agricium")),
            "targetType": present(json!("ResourceType")),
            "status": "present",
        })),
        "instability": single(350.0),
        "resistance": single(0.5),
        "optimalWindowMidpoint": single(0.5),
        "optimalWindowMidpointRandomness": single(0.15),
        "optimalWindowThinness": single(2.0),
        "explosionMultiplier": single(4.0),
        "clusterFactor": single(0.2),
    })
}
