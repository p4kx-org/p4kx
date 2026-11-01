// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each mining and harvestable kind, with every field the schema lists.

use serde_json::json;

pub(super) fn compositions() -> serde_json::Value {
    json!([{
        "guid": "a5000000-0000-4000-8000-000000000001",
        "className": "Asteroid_CType_Iron",
        "recordPath": "libs/foundry/records/mining/rockcompositionpresets/asteroid_ctype_iron",
        "depositName": {"key": "@hud_mining_asteroid_name_5", "en": null, "de": null},
        "minimumDistinctElements": 2,
        "parts": [{
            "mineableElementGuid": "a3000000-0000-4000-8000-000000000001",
            "minPercentage": 30.0,
            "maxPercentage": 70.0,
            "probability": 1.0,
            "curveExponent": 1.0,
            "qualityScale": 1.0
        }]
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
    let harvestable = json!({
        "presetGuid": "a6000000-0000-4000-8000-000000000002",
        "presetClassName": "Mining_Common_Aluminum",
        "entityGuid": "a6000000-0000-4000-8000-000000000003",
        "mineableCompositionGuid": "a5000000-0000-4000-8000-000000000001",
        "miningGlobalParamsGuid": "a6000000-0000-4000-8000-000000000004"
    });
    json!([{
        "guid": "a6000000-0000-4000-8000-000000000001",
        "className": "HPP_Lagrange_A",
        "recordPath": "libs/foundry/records/harvestable/providerpresets/system/stanton/asteroidfield/hpp_lagrange_a",
        "groups": [{
            "groupName": "SpaceShip_Mineables",
            "groupProbability": 6.0,
            "harvestables": [{
                "harvestable": harvestable,
                "setupGuid": null,
                "clusteringGuid": "a6000000-0000-4000-8000-000000000005",
                "relativeProbability": 22.0,
                "geometryTagGuids": ["a6000000-0000-4000-8000-000000000006"]
            }]
        }],
        "areas": [{
            "name": "Acidic",
            "globalModifier": 1.0,
            "objectPresetPaths": ["Libs/ObjectPresets/PU/system/stanton/hurston/acidic_pools_01.opr"],
            "modifiers": [{
                "groupIndex": 0,
                "harvestableIndex": 0,
                "modifier": 0.0,
                "geometryTagGuids": []
            }]
        }]
    }])
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
