// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each NPC and entity spawn kind.

use serde_json::json;

use super::source_values;

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
                    "givenName": source_values::present(json!("text:23:@fixture_location_title")),
                    "givenLastName": absent_name(),
                    "givenNickName": absent_name(),
                    "chanceOfNickName": 0.05,
                    "nameDataGuid": "e8000000-0000-4000-8000-000000000001"
                }
            }]
        }]
    }])
}

fn absent_name() -> serde_json::Value {
    json!({
        "state": "absent", "value": null, "sourceUsesRefs": [],
        "sourceSetComplete": true, "containsEncryptedSource": false
    })
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
