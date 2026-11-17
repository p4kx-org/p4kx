// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each ship spawn and location kind.

use serde_json::json;

use super::source_values;

pub(super) fn location_templates() -> serde_json::Value {
    json!([{
        "guid": "e3000000-0000-4000-8000-000000000001",
        "className": "AEGS_Reclaimer_Space_Stanton2c",
        "recordPath": "libs/foundry/records/missiondata/pu_locations/templates/aegs_reclaimer_space_stanton2c",
        "disabled": false,
        "classification": {
            "state": "absent", "value": null, "sourceUsesRefs": [],
            "sourceSetComplete": true, "containsEncryptedSource": false
        },
        "configuration": {
            "state": "absent", "value": null, "sourceUsesRefs": [],
            "sourceSetComplete": true, "containsEncryptedSource": false
        },
        "isSecurityNetworkHost": null,
        "tags": source_values::present(json!([location_tag("Missions.LocationType.HijackedShip")])),
        "producesTags": source_values::present(json!({
            "positiveTags": source_values::present(json!([])),
            "negativeTags": source_values::present(json!([]))
        })),
        "consumesTags": source_values::present(json!({
            "positiveTags": source_values::present(json!([location_tag("Missions.Produced")])),
            "negativeTags": source_values::present(json!([]))
        })),
        "aiSpawnTags": source_values::present(json!([])),
        "names": [{
            "tag": location_tag("Missions.Text.Name"),
            "displayName": source_values::present(json!("text:23:@fixture_location_title"))
        }],
        "entityClusterMemberGuid": null
    }])
}

fn location_tag(path: &str) -> serde_json::Value {
    json!({
        "path": source_values::present(json!(path)),
        "danglingGuid": null
    })
}

pub(super) fn location_name_text() -> serde_json::Value {
    json!({
        "id": source_values::present(json!("text:23:@fixture_location_title")),
        "key": source_values::present(json!("@fixture_location_title")),
        "translations": source_values::present(json!({
            "en": {
                "sourceText": source_values::present(json!("Fixture outpost")),
                "displayText": source_values::present(json!("Fixture outpost"))
            }
        }))
    })
}

pub(super) fn ship_spawns() -> serde_json::Value {
    json!([{
        "guid": "e1000000-0000-4000-8000-000000000001",
        "allowedForMissionRestrictedDeliveries": false,
        "groups": [{
            "name": null,
            "slots": [{
                "options": [{
                    "weight": 1.0,
                    "concurrentAmount": 1,
                    "includeLocationAISpawnTags": false,
                    "crewInheritsLocationFaction": false,
                    "tags": ["Ship.Model.Cutlass_Black"],
                    "negativeTags": [],
                    "markupTags": [],
                    "entityTags": [],
                    "initialDamageGuid": null,
                    "namePropertyVariable": null
                }]
            }]
        }]
    }])
}

pub(super) fn location_queries() -> serde_json::Value {
    json!([{
        "guid": "e2000000-0000-4000-8000-000000000001",
        "count": "many",
        "minLocations": 1,
        "maxLocations": 3,
        "failIfMinNotFound": true,
        "logErrorOnSearchFail": false,
        "resourceTags": ["Missions.MissionType.KillExclusive"],
        "conditions": [{
            "condition": "tagSearch",
            "tagType": "General",
            "terms": [{
                "positiveTags": ["Missions.LocationType.System.Stanton.Stanton1"],
                "negativeTags": []
            }]
        }, {
            "condition": "excludeNearbyLocations",
            "rangeKm": 100,
            "locationProperty": "Pickup_BP"
        }]
    }])
}
