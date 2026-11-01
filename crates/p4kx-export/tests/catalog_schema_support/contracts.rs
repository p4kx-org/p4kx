// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One contract with a pickup place, a reputation gain, the contract facts and a hauling order,
//! and a cargo manifest.

use serde_json::json;

/// The places, the giver faction and the blueprint rewards of an entry.
fn gaps() -> serde_json::Value {
    json!({
        "offeredAt": {
            "localityGuids": ["a0000000-0000-4000-8000-000000000001"],
            "locations": places(),
            "brokerFallback": false
        },
        "giverFaction": {
            "source": "handler",
            "faction": {
                "guid": "60000000-0000-4000-8000-000000000001",
                "name": "acme",
                "displayName": {"key": "@faction_acme", "en": "Acme Emporium", "de": null}
            }
        },
        "blueprintRewards": [{
            "poolGuid": "e4000000-0000-4000-8000-000000000001",
            "source": "result",
            "chance": 1.0
        }]
    })
}

pub(super) fn contracts() -> serde_json::Value {
    let gains = json!([{
        "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
        "factionReputationName": "acme",
        "scopeGuid": "60000000-0000-4000-8000-000000000002",
        "scopeName": "faction",
        "amount": 200
    }]);
    let mut entry = json!({
        "guid": "20000000-0000-4000-8000-000000000001",
        "subContractGuid": "30000000-0000-4000-8000-000000000002",
        "recordGuid": "10000000-0000-4000-8000-000000000001",
        "recordPath": "libs/foundry/records/contracts/contractgenerator/delivery",
        "debugName": "Deliver_A",
        "handler": "Delivery_Career",
        "templateGuid": "40000000-0000-4000-8000-000000000001",
        "brokerEntryGuid": "b1000000-0000-4000-8000-000000000001",
        "notForRelease": false,
        "workInProgress": null,
        "title": {"key": "@title_a", "en": "Deliver Scrap to [LOCATION]", "de": null},
        "description": {"key": "@desc_handler", "en": "Wanted", "de": null},
        "missionTypeGuid": "50000000-0000-4000-8000-000000000001",
        "missionType": {"key": "@type_delivery", "en": "Delivery", "de": null},
        "contractor": {"key": null, "en": null, "de": null},
        "faction": {
            "guid": "60000000-0000-4000-8000-000000000001",
            "name": "acme",
            "displayName": {"key": "@faction_acme", "en": "Acme Emporium", "de": null}
        },
        "pickupPlaces": places(),
        "reputationGains": gains,
        "minStandingGuid": "62000000-0000-4000-8000-000000000001",
        "maxStandingGuid": null,
        "standingRequirements": [{
            "level": "contract",
            "exclude": false,
            "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
            "scopeGuid": "61000000-0000-4000-8000-000000000001",
            "minStandingGuid": "62000000-0000-4000-8000-000000000001",
            "maxStandingGuid": null
        }],
        "fixedRewards": [{"currency": "UEC", "amount": 40000, "max": 0, "plusBonuses": false}],
        "haulingOrders": orders()
    });
    for extra in [facts(), cooldown(), gaps()] {
        if let (Some(entry), Some(extra)) = (entry.as_object_mut(), extra.as_object()) {
            entry.extend(extra.clone());
        }
    }
    if let (Some(target), serde_json::Value::Object(links)) = (entry.as_object_mut(), spawn_links())
    {
        target.extend(links);
    }
    json!([entry])
}

fn cooldown() -> serde_json::Value {
    json!({
        "cooldownLayer": "contract",
        "cooldownLayers": {
            "hasPersonalCooldown": "handler",
            "personalCooldownTime": "contract",
            "personalCooldownTimeVariation": "handler",
            "abandonedCooldownTime": "handler",
            "abandonedCooldownTimeVariation": null
        },
        "hasPersonalCooldown": true,
        "personalCooldownTime": 2880.0,
        "personalCooldownTimeVariation": 60.0,
        "abandonedCooldownTime": 30.0,
        "abandonedCooldownTimeVariation": null
    })
}

fn places() -> serde_json::Value {
    json!([{
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "objectType": "Planet",
        "place": {"key": "@Stanton1_Name", "en": "Hurston", "de": null}
    }])
}

fn spawn_links() -> serde_json::Value {
    json!({
        "shipSpawns": [{
            "variable": "HostileShips_BP",
            "token": null,
            "level": "contract",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        }],
        "locationQueries": [{
            "variable": "Location_BP",
            "token": "Location",
            "level": "template",
            "queryGuid": "e2000000-0000-4000-8000-000000000001"
        }],
        "locationPrerequisites": [{
            "level": "handler",
            "variable": "Location_BP",
            "token": null,
            "locationLevelType": "Planet"
        }]
    })
}

fn facts() -> serde_json::Value {
    json!({
        "timeToComplete": 20.0,
        "buyInAmount": 0,
        "canBeShared": {"value": true, "layer": "template"},
        "illegal": {"value": false, "layer": "contract"},
        "onceOnly": {"value": true, "layer": "subContract"},
        "maxPlayersPerInstance": {"value": 1, "layer": "handler"},
        "availableInPrison": null,
        "canReacceptAfterAbandoning": {"value": true, "layer": "handler"},
        "canReacceptAfterFailing": null,
        "hideInMobiGlas": null,
        "requiredIntros": [{
            "level": "handler",
            "tagGuid": "90000000-0000-4000-8000-000000000002",
            "introContracts": ["20000000-0000-4000-8000-000000000001"]
        }]
    })
}

fn orders() -> serde_json::Value {
    json!([{
        "source": "contract",
        "variableName": "HaulingOverride",
        "requirements": [{"options": [[{
            "kind": "resource",
            "resourceGuid": "f5000000-0000-4000-8000-000000000001",
            "entityClassGuid": null,
            "entityClassesGuid": null,
            "minScu": 4,
            "maxScu": 8,
            "maxContainerSize": 16,
            "minAmount": null,
            "maxAmount": null
        }]]}]
    }])
}

pub(super) fn manifests() -> serde_json::Value {
    json!([{
        "guid": "f7000000-0000-4000-8000-000000000001",
        "className": "LegalCargo_Even",
        "recordPath": "libs/foundry/records/cargomanifest/civilian/legalcargo_even",
        "fillType": "even",
        "minCapacityRange": 0.45,
        "maxCapacityRange": 0.5,
        "lockCargoGridsOnSpawn": false,
        "resources": [{
            "resourceGuid": "f5000000-0000-4000-8000-000000000001",
            "probability": 1.0,
            "minAllocation": null,
            "maxAllocation": null
        }],
        "spawnRules": [{
            "kind": "entities",
            "probability": 1.0,
            "entityGuids": ["f6000000-0000-4000-8000-000000000001"],
            "resourceGuids": [],
            "quantityAlgorithm": "CargoManifestFixedQuantityDef",
            "quantity": 30,
            "proportion": null
        }]
    }])
}
