// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A sample of the `missionBrokerEntries` kind that holds every field and every union variant.

use serde_json::json;

use super::spawns::{objective_display, objective_handler};

fn no_text() -> serde_json::Value {
    json!({"key": null, "en": null, "de": null})
}

fn modifiers() -> serde_json::Value {
    let header = |modifier: &str| {
        json!({
            "modifier": modifier,
            "modifierName": null,
            "enabled": true,
            "locationProperty": null
        })
    };
    let with = |modifier: &str, fields: serde_json::Value| {
        let mut object = header(modifier);
        if let (Some(target), Some(extra)) = (object.as_object_mut(), fields.as_object()) {
            target.extend(extra.clone());
        }
        object
    };
    json!([
        with(
            "factionHostility",
            json!({
                "factionGuid": "a1000000-0000-4000-8000-000000000001",
                "myReaction": "hostile",
                "theirReaction": "friendly",
                "ignoreCriminalHostility": false
            })
        ),
        with(
            "hostileMission",
            json!({
                "brokerEntryGuid": "b1000000-0000-4000-8000-000000000002",
                "legalToAttack": true
            })
        ),
        with(
            "ignoreMissionPlayerCriminality",
            json!({
                "brokerEntryGuid": "b1000000-0000-4000-8000-000000000002"
            })
        ),
        with(
            "lawLicense",
            json!({"licenseType": "illegalGoodsLicenseClassC"})
        )
    ])
}

fn moment(hour: Option<i64>) -> serde_json::Value {
    json!({
        "year": 2950,
        "month": "december",
        "day": 1,
        "hour": hour,
        "minute": 0,
        "second": 0
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one sample that holds every field of the kind"
)]
pub(super) fn broker_entries() -> serde_json::Value {
    json!([{
        "guid": "b1000000-0000-4000-8000-000000000001",
        "className": "Haul_A",
        "recordPath": "libs/foundry/records/missionbroker/covalex/haul_a",
        "notForRelease": false,
        "debugName": "Haul A",
        "title": {"key": "@haul_title", "en": "Haul", "de": null},
        "hudTitle": no_text(),
        "description": {"key": "@haul_description", "en": "Haul cargo", "de": null},
        "missionGiver": no_text(),
        "commsChannelName": no_text(),
        "missionTypeGuid": "62f92440-202a-43d3-9cb8-b917d4e4a102",
        "locationGuid": null,
        "localityGuid": null,
        "giverGuid": "a2000000-0000-4000-8000-000000000001",
        "giverFragmentTags": "Covalex",
        "module": "Libs/Subsumption/Missions/Haul.xml",
        "modulePerPlayer": false,
        "moduleHandlesOwnShutdown": false,
        "owner": {"guid": "300dcbf0-d201-465b-a7be-af7e15a0a512", "className": "MDT"},
        "difficulty": 3,
        "preShowObjectives": true,
        "showLifeTimeInMobiGlas": true,
        "settings": {
            "canBeShared": true,
            "hasCompleteButton": false,
            "handlesAbandonRequest": false,
            "displayAlliedMarkers": true,
            "onlyOwnerCanComplete": false
        },
        "availability": {
            "initiallyActive": true,
            "showAsOffer": true,
            "requestOnly": false,
            "onceOnly": false,
            "notifyOnAvailable": false,
            "availableInPrison": false,
            "lawfulMission": true,
            "tutorial": false,
            "canReacceptAfterAbandoning": true,
            "canReacceptAfterFailing": true,
            "applyCompletionTagsOnAbandoned": false,
            "applyCompletionTagsOnFailed": false,
            "wantedLevelMin": 0.0,
            "wantedLevelMax": 2.0
        },
        "schedule": [{
            "start": moment(Some(8)),
            "end": moment(Some(17)),
            "repeatEnd": moment(None),
            "repeatType": "repeatYearly"
        }],
        "cooldowns": {
            "hasPersonalCooldown": true,
            "personalCooldownTime": 60.0,
            "personalCooldownTimeVariation": 5.0,
            "abandonedCooldownTime": 30.0,
            "abandonedCooldownTimeVariation": 0.0,
            "respawnTime": 10.0,
            "respawnTimeVariation": 1.0
        },
        "instances": {
            "maxInstances": 4,
            "maxInstancesPerPlayer": 1,
            "maxPlayersPerInstance": 4,
            "hasLifeTime": true,
            "lifeTime": 120.0,
            "lifeTimeVariation": 10.0
        },
        "reward": {
            "currency": "UEC",
            "reward": 12000,
            "max": 15000,
            "plusBonuses": true,
            "reputationBonusGuid": null
        },
        "buyInAmount": 500,
        "refundBuyInOnWithdraw": true,
        "finish": {
            "failIfSentToPrison": true,
            "failIfBecameCriminal": true,
            "failIfLeavePrison": false,
            "autoEnd": true,
            "completionTime": 45.5,
            "endReason": no_text(),
            "resultAfterTimerEnd": "failed",
            "remainingTimeToShowTimer": 5.0
        },
        "invitationMissionGuid": "b1000000-0000-4000-8000-000000000002",
        "linkedMissionGuid": null,
        "associatedMissionGuids": ["b1000000-0000-4000-8000-000000000002"],
        "requiredMissionGuids": [],
        "unlessAvailableMissionGuids": [],
        "completionTags": ["Missions.Done"],
        "missionTags": [],
        "requiredAreaTags": [],
        "excludedAreaTags": [],
        "requiredCompletedTags": [{"positiveTags": ["Missions.Done"], "negativeTags": []}],
        "journalEntriesOnComplete": ["a3000000-0000-4000-8000-000000000001"],
        "journalEntriesOnRemove": [],
        "standingRequirements": [{
            "type": "requirement",
            "comparison": "greaterThanOrEqualTo",
            "factionReputationGuid": "a4000000-0000-4000-8000-000000000001",
            "scopeGuid": "a4000000-0000-4000-8000-000000000002",
            "standingGuid": "a4000000-0000-4000-8000-000000000003"
        }, {"type": "and"}, {"type": "leftParenthesis"}, {"type": "rightParenthesis"}],
        "reputationResults": [{"index": 0, "amounts": [{
            "factionReputationGuid": "a4000000-0000-4000-8000-000000000001",
            "scopeGuid": "a4000000-0000-4000-8000-000000000002",
            "rewardGuid": "a4000000-0000-4000-8000-000000000004",
            "amount": 2
        }]}],
        "missionProperties": [{
            "variable": "Place_BP",
            "token": "Place",
            "valueType": "Location",
            "value": {"type": "location", "queryGuid": "e2000000-0000-4000-8000-000000000001"}
        }],
        "shipSpawns": [{
            "variable": "Wave_BP",
            "token": null,
            "level": "broker",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        }],
        "npcSpawns": [{
            "variable": "Crew_BP",
            "token": null,
            "level": "broker",
            "poolGuid": "e5000000-0000-4000-8000-000000000001"
        }],
        "entitySpawns": [{
            "variable": "Loot_BP",
            "token": null,
            "level": "broker",
            "poolGuid": "e6000000-0000-4000-8000-000000000001"
        }],
        "locationQueries": [],
        "haulingOrders": [],
        "objectives": [{
            "id": "ce628b01-6ec2-4add-9f47-078493d1f48e",
            "parentId": null,
            "debugName": "Haul",
            "startsActive": true,
            "variableName": null,
            "weight": null,
            "phaseTag": null,
            "handler": objective_handler(),
            "display": objective_display(),
            "overrideDisplay": {"titleOverride": no_text(), "descriptionOverride": no_text()},
            "rewardContribution": null,
            "objectiveProperties": [],
            "comms": []
        }],
        "partialPayout": null,
        "modifiers": modifiers(),
        "missionFlow": [{
            "note": null,
            "condition": {"type": "propertyIsSet", "propertyVariable": "Place_BP"},
            "action": {"type": "stateChange", "toState": "completed"}
        }]
    }])
}
