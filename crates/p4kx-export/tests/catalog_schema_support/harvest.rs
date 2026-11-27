// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each mining and harvestable kind, with every field the schema lists.

use serde_json::json;

pub(super) fn compositions() -> serde_json::Value {
    let present = super::source_values::present;
    let guid = "a5000000-0000-4000-8000-000000000001";
    let path = "libs/foundry/records/mining/rockcompositionpresets/asteroid_ctype_iron";
    let single = |finite: f32| {
        present(json!({
            "type": "f32", "finite": finite, "rawBits": format!("{:08x}", finite.to_bits()),
        }))
    };
    json!([{
        "guid": present(json!(guid)),
        "recordName": present(json!("MineableComposition.Asteroid_CType_Iron")),
        "className": present(json!("Asteroid_CType_Iron")),
        "recordPath": present(json!(path)),
        "sourceType": present(json!("MineableComposition")),
        "fileName": present(json!(format!("{path}.xml"))),
        "recordIndex": present(json!(2)),
        "metadataTag": super::source_values::source(p4kx_extraction::SourceValue::Absent),
        "depositName": present(json!("text:fixture:deposit")),
        "minimumDistinctElements": present(json!({"type": "int32", "value": 2})),
        "parts": present(json!([present(json!({
            "mineableElement": present(json!({
                "targetRecordIndex": present(json!(1)),
                "guid": present(json!("a4000000-0000-4000-8000-000000000001")),
                "recordPath": present(json!("libs/foundry/records/mining/mineableelements/iron")),
                "targetType": present(json!("MineableElement")),
                "status": "present",
            })),
            "minPercentage": single(30.0),
            "maxPercentage": single(70.0),
            "probability": single(1.0),
            "curveExponent": single(1.0),
            "qualityScale": single(1.0),
        }))])),
    }])
}

pub(super) fn mineable_entities() -> serde_json::Value {
    let present = super::source_values::present;
    let class = "MineableRock_AsteroidLegendary_Quantainium";
    let path = "libs/foundry/records/entities/mineable/mineablerock_asteroidlegendary_quantainium";
    json!({
        "availability": "available",
        "rows": [{
            "guid": present(json!("a6000000-0000-4000-8000-000000000003")),
            "recordName": present(json!(format!("EntityClassDefinition.{class}"))),
            "className": present(json!(class)),
            "recordPath": present(json!(path)),
            "sourceType": present(json!("EntityClassDefinition")),
            "fileName": present(json!(format!("{path}.xml"))),
            "recordIndex": present(json!(6)),
            "metadataTag": super::source_values::source(p4kx_extraction::SourceValue::Absent),
            "componentSlots": present(json!(0)),
            "mining": [],
            "radar": []
        }]
    })
}

pub(super) fn presets() -> serde_json::Value {
    let present = super::source_values::present;
    let empty = || super::source_values::source(p4kx_extraction::SourceValue::Absent);
    let link = |index: u32, guid: &str, path: &str, source_type: &str| {
        present(json!({
            "targetRecordIndex": present(json!(index)),
            "guid": present(json!(guid)),
            "recordPath": present(json!(path)),
            "targetType": present(json!(source_type)),
            "status": "present",
        }))
    };
    let entity_guid = "a6000000-0000-4000-8000-000000000003";
    let entity_path = "libs/foundry/records/entities/mineable/mineablerock_asteroid";
    let provider_path = "libs/foundry/records/harvestable/providerpresets/hpp_lagrange_a";
    json!([{
        "guid": present(json!("a6000000-0000-4000-8000-000000000001")),
        "recordName": present(json!("HarvestableProviderPreset.HPP_Lagrange_A")),
        "className": present(json!("HPP_Lagrange_A")),
        "recordPath": present(json!(provider_path)),
        "sourceType": present(json!("HarvestableProviderPreset")),
        "fileName": present(json!(format!("{provider_path}.xml"))),
        "recordIndex": present(json!(4)),
        "metadataTag": empty(),
        "groups": present(json!([present(json!({
            "groupName": present(json!("SpaceShip_Mineables")),
            "groupProbability": single(6.0),
            "harvestables": present(json!([present(json!({
                "harvestable": link(5, "a6000000-0000-4000-8000-000000000002",
                    "libs/foundry/records/harvestable/mining_common_aluminum", "HarvestablePreset"),
                "harvestableEntityClass": empty(),
                "harvestableSetup": empty(),
                "clustering": empty(),
                "relativeProbability": single(22.0),
                "geometries": present(json!([])),
                "spawn": {
                    "entitySelection": present(json!("presetFallback")),
                    "presetClassName": present(json!("Mining_Common_Aluminum")),
                    "presetEntityClass": link(6, entity_guid, entity_path, "EntityClassDefinition"),
                    "entity": present(json!({
                        "recordIndex": present(json!(6)),
                        "guid": present(json!(entity_guid)),
                        "miningParams": present(json!({
                            "componentIndex": present(json!(0)),
                            "composition": link(2, "a5000000-0000-4000-8000-000000000001",
                                "libs/foundry/records/mining/rockcompositionpresets/asteroid_ctype_iron",
                                "MineableComposition"),
                            "globalParams": empty(),
                        })),
                    })),
                },
            }))])),
        }))])),
        "areas": provider_areas(),
    }])
}

fn single(finite: f32) -> serde_json::Value {
    super::source_values::present(json!({
        "type": "f32", "finite": finite, "rawBits": format!("{:08x}", finite.to_bits()),
    }))
}

fn provider_areas() -> serde_json::Value {
    let present = super::source_values::present;
    present(json!([present(json!({
        "name": present(json!("Acidic")),
        "globalModifier": single(1.0),
        "areaType": present(json!({
            "sourceType": present(json!("HarvestableAreaTypeObjectPreset")),
            "objectPresetPaths": present(json!([present(json!("Libs/ObjectPresets/example.opr"))])),
        })),
        "modifiers": present(json!([])),
    }))]))
}

pub(super) fn multi_configs() -> serde_json::Value {
    json!([{
        "guid": "a7000000-0000-4000-8000-000000000001",
        "className": "SlotPreset_Cave",
        "recordPath": "libs/foundry/records/harvestable/slotpresets/caves/slotpreset_cave",
        "defaultConfigGuid": null,
        "fieldOrigins": {},
        "ignoreAttachableTags": false,
        "taggedConfigs": [{
            "name": "Rock",
            "tagGuids": ["a7000000-0000-4000-8000-000000000002"],
            "configGuid": null,
            "respawnTimeMultiplier": 1.0,
            "initialSlotsProbability": 0.5,
            "subHarvestables": [{
                "harvestable": null,
                "setupGuid": null,
                "respawnTimeMultiplier": 1.0,
                "relativeProbability": 2.0,
                "relativeProbabilityDeepest": null,
                "geometryTagGuids": [],
                "lootConfig": {
                    "lootTableGuid": "a9000000-0000-4000-8000-000000000005",
                    "lootTableV3Guid": "a9000000-0000-4000-8000-000000000001",
                    "poolFilterGuid": "a9000000-0000-4000-8000-000000000003",
                    "totalResultsLimit": 4,
                    "chanceToGenerate": 1.0,
                    "chanceToGenerateAdditionalInventories": 0.5,
                    "fullness": {"min": 0.2, "max": 0.8}
                }
            }]
        }]
    }])
}

pub(super) fn locations() -> serde_json::Value {
    json!([{
        "presetGuid": "a6000000-0000-4000-8000-000000000001",
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "system": "stanton",
        "container": "Data\\ObjectContainers\\PU\\system\\stanton\\lagrangepoints\\childclouds\\childcloud_s1_l1_01.socpak"
    }])
}

pub(super) fn shops() -> serde_json::Value {
    let present = super::source_values::present;
    json!([{
        "key": present(json!("4a269932-183e-362d-aacc-188e7debbea9,4ce1f681-b479-5269-0536-3d7f02fdccaf,41000000-0000-4000-8000-000000000001,42eef9e5-ecd4-6e5b-6254-e332a5dd44b9")),
        "entityGuid": present(json!("42eef9e5-ecd4-6e5b-6254-e332a5dd44b9")),
        "system": present(json!("stanton")),
        "entityName": present(json!("SCShop_CommEx_TDD_NewBabbage")),
        "registryName": present(json!("L_TDD_NewBabbage")),
        "defaultValue": present(json!("d0000000-0000-4000-8000-000000000001")),
        "containers": present(json!([
            present(json!("OOC_Stanton")),
            present(json!("OOC_Stanton_4_Microtech")),
            present(json!("NewBabbage_LOC")),
        ])),
        "body": present(json!("OOC_Stanton_4_Microtech")),
        "location": present(json!("NewBabbage_LOC")),
        "starmapObjectGuid": present(json!("a2000000-0000-4000-8000-000000000001")),
        "kind": present(json!("commex")),
        "brand": present(json!("tdd")),
        "commodityExchange": present(json!(true)),
        "inventory": present(json!("inv_commex_newbabbage")),
        "itemGuids": present(json!([present(json!("d195eb47-24b8-45c0-a4d0-1771d60dc1a8"))]))
    }])
}
