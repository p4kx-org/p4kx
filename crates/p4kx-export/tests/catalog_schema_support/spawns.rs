// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each ship spawn and location kind.

use serde_json::json;

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
        "tags": ["Missions.LocationType.HijackedShip"],
        "producesTags": {"positiveTags": [], "negativeTags": []},
        "consumesTags": {"positiveTags": ["Missions.Produced"], "negativeTags": []},
        "aiSpawnTags": [],
        "names": [{
            "tag": "Missions.Text.Name",
            "displayName": {"key": "@vehicle_NameAEGS_Reclaimer", "en": "Aegis Reclaimer", "de": null}
        }],
        "entityClusterMemberGuid": null
    }])
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
