// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The blueprint pools and the crafting tiers.

use serde_json::json;

pub(super) fn pools() -> serde_json::Value {
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

pub(super) fn tiers() -> serde_json::Value {
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
