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

#[expect(
    clippy::too_many_lines,
    reason = "one sample that holds every field of the kind"
)]
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
            "resultAfterTimerEnd": "failed",
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
            "variableName": null,
            "weight": null,
            "phaseTag": null,
            "handler": objective_handler(),
            "display": objective_display(),
            "overrideDisplay": {
                "titleOverride": {"key": null, "en": null, "de": null},
                "descriptionOverride": {"key": null, "en": null, "de": null}
            },
            "rewardContribution": {
                "structName": "ObjectiveRewardContribution_PercentOfMission",
                "percentContribution": 100,
                "doPartialRewardPayout": true
            },
            "objectiveProperties": objective_properties(),
            "comms": [{
                "trigger": "completed",
                "timing": "simultaneous",
                "delay": 0,
                "communicationTag": "Comms.Tag"
            }]
        }],
        "partialPayout": {"thresholds": [{
            "minPercentage": 76,
            "maxPercentage": 99,
            "currencyRewardMultiplier": 0.76,
            "reputationMultipliers": [{
                "scopeGuid": "d1000000-0000-4000-8000-000000000001",
                "reputationRewardMultiplier": 0.76
            }]
        }]},
        "modifiers": [{
            "modifier": "securityClearance",
            "modifierName": null,
            "enabled": true,
            "locationProperty": "Place_BP",
            "clearanceTokenGuid": "ed53fbee-2677-4746-b6f4-dc9aded2d544"
        }, {
            "modifier": "requestRentShip",
            "modifierName": "RentalMiner",
            "enabled": true,
            "locationProperty": null,
            "itemGuid": null,
            "durationSeconds": 3600,
            "clearRentalOnFail": true
        }, {
            "modifier": "shipKioskFilter",
            "modifierName": null,
            "enabled": true,
            "locationProperty": null,
            "terms": [{"positiveTags": ["Ship.Model.Mole"], "negativeTags": []}]
        }, {
            "modifier": "other",
            "structName": "MissionModifier_Unknown",
            "locationProperty": null,
            "modifierName": null,
            "enabled": null
        }],
        "owner": {"guid": "300dcbf0-d201-465b-a7be-af7e15a0a512", "className": "MDT"},
        "missionFlow": [{
            "note": "When [Hauling] completed -> Mission Completed",
            "condition": {"type": "and", "conditions": [
                {"type": "tokenState", "objectiveId": "ce628b01-6ec2-4add-9f47-078493d1f48e", "tokenState": "completed"},
                {"type": "not", "condition": {"type": "propertyIsSet", "propertyVariable": "Flag_BP"}},
                {"type": "or", "conditions": [{"type": "allTokensState", "tokenState": "failed"}]},
                {"type": "other", "structName": "MissionFlowCondition_Unknown"}
            ]},
            "action": {"type": "stateChange", "toState": "completed"}
        }, {
            "note": null,
            "condition": null,
            "action": {"type": "tokenStateChange", "objectiveId": null, "toState": "active"}
        }, {
            "note": null,
            "condition": null,
            "action": {"type": "pickRandomMissionPhase", "toState": null, "phases": [
                {"objectiveId": "ce628b01-6ec2-4add-9f47-078493d1f48e", "probability": 0.5}
            ]}
        }, {
            "note": null,
            "condition": null,
            "action": {"type": "other", "structName": "MissionFlowAction_Unknown"}
        }],
        "startComms": {"delay": 0, "communicationTag": "Comms.Tag"},
        "endComms": [{"reason": "complete", "delay": 0, "communicationTag": null}]
    }])
}

fn objective_display() -> serde_json::Value {
    json!({
        "category": "primary",
        "hideOnHud": false,
        "markerLabel": {"key": null, "en": null, "de": null},
        "shortDescription": {"key": "@obj_short", "en": "Deliver", "de": null},
        "longDescription": {"key": null, "en": null, "de": null}
    })
}

fn objective_handler() -> serde_json::Value {
    json!({
        "structName": "ObjectiveHandler_Hauling",
        "module": "Libs/Subsumption/Missions/GoTo.xml",
        "moduleHierarchyGuid": null,
        "disableTravelObjectives": false,
        "disableReturnObjectives": false,
        "travelRadiusKm": 5.0,
        "allPlayersLeftGracePeriod": 30.0,
        "locationProperty": "Place_BP",
        "organizationProperty": null,
        "haulingLimitProperty": null,
        "targetProperty": null,
        "minDistance": null,
        "maxDistance": null,
        "hideMarkers": false,
        "securityManifestGuid": null,
        "navPoint": {
            "entityClassGuid": null,
            "displayName": {"key": null, "en": null, "de": null},
            "extendedTextTokens": []
        },
        "travelObjective": objective_display(),
        "returnObjective": null,
        "hauling": {
            "itemToken": "item",
            "pickupLocationToken": null,
            "amountToken": null,
            "totalToken": null,
            "dropOffLocationToken": null,
            "andObjective": null,
            "orObjective": null,
            "collectObjective": null,
            "collectAndDeliverObjective": null,
            "resourceDeliverObjective": null,
            "itemDeliverObjective": objective_display(),
            "listItemObjective": null
        },
        "instance": {
            "completePhaseOnPlayerEnter": true,
            "enterTravelRadiusKm": 4.0,
            "hudObjectiveText": {"key": null, "en": null, "de": null},
            "tagIdentifier": "Instance.Tag",
            "searchTags": [],
            "objectiveMarker": null
        }
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one sample of every kind of value an objective property holds"
)]
fn objective_properties() -> serde_json::Value {
    json!([{
        "kind": "referenced",
        "variable": "Place_BP",
        "token": "Place",
        "contractProperty": "Place_BP",
        "value": null,
        "linkedOutput": null
    }, {
        "kind": "input",
        "variable": "In_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "deliveryOrder"},
        "linkedOutput": {
            "objectiveId": "ce628b01-6ec2-4add-9f47-078493d1f48e",
            "variable": "Out_BP"
        }
    }, {
        "kind": "embedded",
        "variable": "Mood_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "stringHash", "options": [{
            "displayName": {"key": "@mood", "en": "Calm", "de": null},
            "value": "calm",
            "weighting": 1,
            "dependentProperties": ["Place_BP"]
        }]},
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Count_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "integer", "options": [{
            "displayName": {"key": null, "en": null, "de": null},
            "value": 3.0,
            "variation": 1.0,
            "weighting": 1,
            "dependentProperties": []
        }]},
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Ships_BP",
        "token": null,
        "contractProperty": null,
        "value": {
            "type": "shipSpawnDescriptions",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        },
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Tags_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "tags", "tags": ["Ship.Model.Mole"], "negativeTags": []},
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Where_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "location", "queryGuid": "e2000000-0000-4000-8000-000000000001"},
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Crew_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "other", "structName": "MissionPropertyValue_NPCSpawnDescriptions"},
        "linkedOutput": null
    }, {
        "kind": "output",
        "variable": "Out_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "boolean", "value": true},
        "linkedOutput": null
    }, {
        "kind": "output",
        "variable": "Obj_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "object"},
        "linkedOutput": null
    }, {
        "kind": "output",
        "variable": "Float_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "float", "options": []},
        "linkedOutput": null
    }])
}
