// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each NPC and entity spawn kind.

use serde_json::json;

/// An embedded objective property named `variable` that holds `value`.
pub(super) fn embedded_property(variable: &str, value: &serde_json::Value) -> serde_json::Value {
    json!({
        "kind": "embedded",
        "variable": variable,
        "token": null,
        "contractProperty": null,
        "value": value,
        "linkedOutput": null
    })
}

pub(super) fn npc_spawns() -> serde_json::Value {
    json!([{
        "guid": "e5000000-0000-4000-8000-000000000001",
        "groups": [{
            "name": "SC_CE -> DA_I (Grenadier x1)",
            "options": [{
                "priority": 1,
                "includeLocationAISpawnTags": false,
                "weight": 0.5,
                "autoSpawnSettings": {
                    "name": "SC_CE -> DA_I (Grenadier x1)",
                    "initialActivity": "Sentry",
                    "excludeShipCrew": true,
                    "excludeSpawnGender": "Female",
                    "minGroupSize": -1,
                    "maxGroupSize": 4,
                    "maxConcurrentSpawns": 4,
                    "maxSpawns": 4,
                    "minSpawnDelay": 0,
                    "maxSpawnDelay": 10,
                    "missionAlliedMarker": false,
                    "isCritical": true,
                    "characterTags": {
                        "positiveTags": ["AI.Character.Grenadier"],
                        "negativeTags": []
                    },
                    "closetTags": {"positiveTags": [], "negativeTags": []},
                    "roomTags": {"positiveTags": [], "negativeTags": []},
                    "defendAreaTags": {"positiveTags": [], "negativeTags": []},
                    "scheduleAreaTags": {"positiveTags": [], "negativeTags": []},
                    "entityTags": [],
                    "inventoryItems": [{
                        "entityClassGuid": "30000000-0000-4000-8000-000000000001",
                        "quantity": 1,
                        "amountInventoriesToFill": 1,
                        "markAsMissionItem": true
                    }],
                    "factionOverrideGuid": null
                },
                "identifierTags": ["AI.Identifier.Boss"],
                "namePropertyVariable": null,
                "embeddedName": {
                    "randomName": true,
                    "randomLastName": true,
                    "randomNickName": false,
                    "givenName": {"key": "@mg_miles_capturegang_marker_001", "en": "Marker", "de": null},
                    "givenLastName": {"key": null, "en": null, "de": null},
                    "givenNickName": {"key": null, "en": null, "de": null},
                    "chanceOfNickName": 0.05,
                    "nameDataGuid": "e8000000-0000-4000-8000-000000000001"
                }
            }]
        }]
    }])
}

pub(super) fn entity_spawns() -> serde_json::Value {
    json!([{
        "guid": "e6000000-0000-4000-8000-000000000001",
        "groups": [{
            "name": "HarDrive",
            "slots": [{
                "options": [{
                    "amount": 1,
                    "weight": 1.0,
                    "tags": ["Mission.Item.HardDrive"],
                    "negativeTags": [],
                    "markupTags": [],
                    "entityTags": []
                }]
            }]
        }]
    }])
}
