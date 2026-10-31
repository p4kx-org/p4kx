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
        }, {
            "condition": "other",
            "structName": "DataSetMatchCondition_Unknown"
        }]
    }])
}

pub(super) fn contract_templates() -> serde_json::Value {
    json!([{
        "guid": "e4000000-0000-4000-8000-000000000001",
        "className": "AcePilot",
        "recordPath": "libs/foundry/records/contracts/contracttemplates/acepilot",
        "notForRelease": false,
        "workInProgress": false,
        "title": {"key": "@acepilot_title", "en": "Ace Pilot", "de": null},
        "missionTypeGuid": "62f92440-202a-43d3-9cb8-b917d4e4a102",
        "illegal": false,
        "showLifeTimeInMobiGlas": true,
        "preShowObjectives": false,
        "classType": "ContractClass_Contract",
        "settings": {
            "canBeShared": false,
            "hasCompleteButton": false,
            "handlesAbandonRequest": false,
            "displayAlliedMarkers": true,
            "onlyOwnerCanComplete": false
        },
        "autoFinish": {
            "failIfSentToPrison": true,
            "failIfBecameCriminal": true,
            "failIfLeavePrison": false,
            "completionTime": 300,
            "autoEnd": true,
            "resultAfterTimerEnd": "Failed",
            "remainingTimeToShowTimer": 15
        },
        "contractProperties": [{
            "variable": "AcePilot_BP",
            "token": null,
            "valueType": "ShipSpawnDescriptions"
        }],
        "shipSpawns": [{
            "variable": "AcePilot_BP",
            "token": null,
            "level": "template",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        }],
        "locationQueries": [],
        "haulingOrders": [],
        "objectives": [{
            "id": "ce628b01-6ec2-4add-9f47-078493d1f48e",
            "parentId": null,
            "debugName": "AcePilot",
            "startsActive": true,
            "handlerType": "ObjectiveHandler_Local",
            "phaseTag": null
        }]
    }])
}
