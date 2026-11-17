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
    json!([{
        "guid": "a6000000-0000-4000-8000-000000000003",
        "className": "MineableRock_AsteroidLegendary_Quantainium",
        "recordPath": "libs/foundry/records/entities/mineable/mineablerock_asteroidlegendary_quantainium",
        "mineableCompositionGuid": "a5000000-0000-4000-8000-000000000001",
        "miningGlobalParamsGuid": "a6000000-0000-4000-8000-000000000004",
        "scanSignature": 3170.0,
        "signatures": [0.0, 0.0, 0.0, 0.0, 3170.0, 0.0, 0.0, 0.0],
        "radarContactTypeGuid": "a6000000-0000-4000-8000-000000000007"
    }])
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

pub(super) fn mining_global_params() -> serde_json::Value {
    json!({
        "guid": "a8000000-0000-4000-8000-000000000001",
        "className": "miningglobalparamsship",
        "recordPath": "libs/foundry/records/mining/miningglobalparamsship",
        "absorbableVolumeThreshold": 330.0,
        "cScuPerVolume": 0.7,
        "childRockInactivityLifetime": 600.0,
        "childRockInvulnerabilityTime": 10.0,
        "childRockLifeTimer": 600.0,
        "childRockZeroGDamping": 0.154_321,
        "compositionMassMultiplier": 1.5,
        "controlledBreakingDecayRate": 0.5,
        "controlledBreakingFillRate": 2.0,
        "controlledBreakingFillRateDanger": 0.7,
        "dangerBreakingDecayRate": 0.3,
        "dangerBreakingFillRate": 2.0,
        "dangerBreakingFillRateExponent": 2.0,
        "dangerToGadgetDamage": 12.0,
        "decayPerMass": 0.2,
        "defaultMass": 100.0,
        "gadgetDestroyThreshold": 0.5,
        "gadgetDetachThreshold": 0.25,
        "modifierPersistenceTime": 0.25,
        "optimalWindowFactor": 0.75,
        "optimalWindowMaxSize": 0.7,
        "optimalWindowSize": 0.5,
        "optimalWindowThinnessCurveFactor": 1.0,
        "powerCapacityPerMass": 10.0,
        "resistanceCurveFactor": 0.6,
        "terrainFactorStaticThreshold": 0.4,
        "hitConsistency": {
            "extractionMagnitude": 1.0,
            "hitHistoryWindow": 4.0,
            "maxEffectOnInstability": 0.5,
            "minDeviation": 0.1,
            "standardDeviationMultiplier": 10.0,
            "timeExponent": 2.0
        },
        "instability": {"curveFactor": 1.0, "wavePeriod": 3.0, "waveVariance": 1.0},
        "explosion": {"dangerPoolFactor": 560.0, "defaultVolume": 5_122.499},
        "wasteResourceTypeGuid": "b1000000-0000-4000-8000-000000000001"
    })
}

pub(super) fn shops() -> serde_json::Value {
    json!([{
        "key": "4a269932-183e-362d-aacc-188e7debbea9,4ce1f681-b479-5269-0536-3d7f02fdccaf,41000000-0000-4000-8000-000000000001,42eef9e5-ecd4-6e5b-6254-e332a5dd44b9",
        "entityGuid": "42eef9e5-ecd4-6e5b-6254-e332a5dd44b9",
        "system": "stanton",
        "entityName": "SCShop_CommEx_TDD_NewBabbage",
        "registryName": "L_TDD_NewBabbage",
        "defaultValue": "d0000000-0000-4000-8000-000000000001",
        "containers": ["OOC_Stanton", "OOC_Stanton_4_Microtech", "NewBabbage_LOC"],
        "body": "OOC_Stanton_4_Microtech",
        "location": "NewBabbage_LOC",
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "kind": "commex",
        "brand": "tdd",
        "commodityExchange": true,
        "inventory": "inv_commex_newbabbage",
        "itemGuids": ["d195eb47-24b8-45c0-a4d0-1771d60dc1a8"]
    }])
}
