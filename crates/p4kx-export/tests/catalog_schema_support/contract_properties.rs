// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One contract-level property of each type of value.

use serde_json::json;

pub(super) fn template_properties() -> serde_json::Value {
    json!([{
        "variable": "AcePilot_BP",
        "token": null,
        "valueType": "ShipSpawnDescriptions",
        "value": {
            "type": "shipSpawnDescriptions",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        }
    }, {
        "variable": "Item_BP",
        "token": null,
        "valueType": "MissionItem",
        "value": {
            "type": "missionItem",
            "conditions": [{
                "condition": "specificItems",
                "itemGuids": ["e8000000-0000-4000-8000-000000000001"]
            }],
            "forcedItemGuid": null,
            "minItemsToFind": 1,
            "maxItemsToFind": 1
        }
    }, {
        "variable": "Name_BP",
        "token": null,
        "valueType": "AIName",
        "value": {
            "type": "aiName",
            "randomName": true,
            "randomLastName": true,
            "randomNickName": false,
            "givenName": {"key": null, "en": null, "de": null},
            "givenLastName": {"key": null, "en": null, "de": null},
            "givenNickName": {"key": null, "en": null, "de": null},
            "chanceOfNickName": 0.05,
            "nameDataGuid": "e8000000-0000-4000-8000-000000000001"
        }
    }, {
        "variable": "Combined_BP",
        "token": null,
        "valueType": "CombinedDataSetEntries",
        "value": {"type": "combinedDataSetEntries", "entryProperties": ["Place_BP"]}
    }, {
        "variable": "Cargo_BP",
        "token": null,
        "valueType": "HaulingOrders",
        "value": {"type": "haulingOrders", "requirements": [{"options": [[{
            "kind": "resource",
            "resourceGuid": "f5000000-0000-4000-8000-000000000001",
            "entityClassGuid": null,
            "entityClassesGuid": null,
            "minScu": 4,
            "maxScu": 8,
            "maxContainerSize": 16,
            "minAmount": null,
            "maxAmount": null
        }]]}]}
    }, {
        "variable": "Pay_BP",
        "token": null,
        "valueType": "Reward",
        "value": {"type": "reward", "reward": {
            "currency": "UEC",
            "reward": 900,
            "max": 1200,
            "plusBonuses": true,
            "reputationBonusGuid": null
        }}
    }, {
        "variable": "Race_BP",
        "token": null,
        "valueType": "TimeTrialRace",
        "value": {"type": "timeTrialRace"}
    }, {
        "variable": "Map_BP",
        "token": null,
        "valueType": "StarMapLocation",
        "value": {"type": "starMapLocation"}
    }])
}
