// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One contract with a pickup place, a reputation gain, the contract facts and a hauling order,
//! and a cargo manifest.

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

/// The places, the giver faction and the blueprint rewards of an entry.
fn gaps() -> Value {
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

pub(super) fn contracts() -> Value {
    let gains = present(json!([{
        "factionReputationGuid": present(json!("60000000-0000-4000-8000-000000000001")),
        "factionReputationName": present(json!("acme")),
        "scopeGuid": present(json!("60000000-0000-4000-8000-000000000002")),
        "scopeName": present(json!("faction")),
        "amount": present(json!("200"))
    }]));
    let mut entry = json!({
        "source": occurrence(),
        "guid": present(json!("20000000-0000-4000-8000-000000000001")),
        "subContractGuid": present(json!("30000000-0000-4000-8000-000000000002")),
        "debugName": "Deliver_A",
        "handler": "Delivery_Career",
        "templateGuid": "40000000-0000-4000-8000-000000000001",
        "brokerEntryGuid": "b1000000-0000-4000-8000-000000000001",
        "propertySetGuids": ["a7000000-0000-4000-8000-000000000001"],
        "notForRelease": false,
        "workInProgress": null,
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
        "fixedRewards": present(json!([{
            "currency": present(json!("UEC")),
            "amount": present(json!("40000")),
            "max": present(json!("0")),
            "plusBonuses": present(json!(false))
        }])),
        "haulingOrders": orders()
    });
    for extra in [facts(), cooldown(), gaps()] {
        if let (Some(entry), Some(extra)) = (entry.as_object_mut(), extra.as_object()) {
            entry.extend(extra.clone());
        }
    }
    if let (Some(target), Value::Object(links)) = (entry.as_object_mut(), spawn_links()) {
        target.extend(links);
    }
    json!({"availability":"available", "rows":[entry]})
}

pub(super) fn titles() -> Value {
    json!({"availability":"available", "rows":[{
        "contract": occurrence(),
        "title": {
            "key": present(json!("fixture-protected-title")),
            "translations": {"en": present(json!("Deliver Scrap to [LOCATION]"))}
        },
        "description": {
            "key": source(SourceValue::Absent),
            "translations": {}
        }
    }]})
}

fn occurrence() -> Value {
    json!({
        "inputRef":"fixture-generator",
        "recordIndex":0,
        "handler":{"valueIndex":"0"},
        "contract":{"valueIndex":"1"},
        "subContract":{"valueIndex":"2"}
    })
}

fn cooldown() -> Value {
    json!({
        "cooldownLayer": present(json!("contract")),
        "cooldownLayers": {
            "hasPersonalCooldown": present(json!("handler")),
            "personalCooldownTime": present(json!("contract")),
            "personalCooldownTimeVariation": present(json!("handler")),
            "abandonedCooldownTime": present(json!("handler")),
            "abandonedCooldownTimeVariation": source(SourceValue::Absent)
        },
        "hasPersonalCooldown": present(json!(true)),
        "personalCooldownTime": present(json!(2880.0)),
        "personalCooldownTimeVariation": present(json!(60.0)),
        "abandonedCooldownTime": present(json!(30.0)),
        "abandonedCooldownTimeVariation": source(SourceValue::Absent)
    })
}

fn places() -> Value {
    json!([{
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "objectType": "Planet",
        "place": {"key": "@Stanton1_Name", "en": "Hurston", "de": null}
    }])
}

fn spawn_links() -> Value {
    json!({
        "shipSpawns": [{
            "variable": "HostileShips_BP",
            "token": null,
            "level": "contract",
            "poolGuid": "e1000000-0000-4000-8000-000000000001"
        }],
        "npcSpawns": [{
            "variable": "Crew_BP",
            "token": null,
            "level": "contract",
            "poolGuid": "e5000000-0000-4000-8000-000000000001"
        }],
        "entitySpawns": [{
            "variable": "Loot_BP",
            "token": null,
            "level": "handler",
            "poolGuid": "e6000000-0000-4000-8000-000000000001"
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

fn facts() -> Value {
    json!({
        "timeToComplete": present(json!(20.0)),
        "buyInAmount": present(json!(0)),
        "canBeShared": layered(Some((json!(true), "template"))),
        "illegal": layered(Some((json!(false), "contract"))),
        "onceOnly": layered(Some((json!(true), "subContract"))),
        "maxPlayersPerInstance": layered(Some((json!(1), "handler"))),
        "availableInPrison": layered(None),
        "canReacceptAfterAbandoning": layered(Some((json!(true), "handler"))),
        "canReacceptAfterFailing": layered(None),
        "hideInMobiGlas": layered(None),
        "requiredIntros": present(json!([{
            "level": present(json!("handler")),
            "tagGuid": present(json!("90000000-0000-4000-8000-000000000002")),
            "introContracts": present(json!([present(json!("20000000-0000-4000-8000-000000000001"))]))
        }]))
    })
}

fn layered(fact: Option<(Value, &str)>) -> Value {
    let (value, layer) = fact.map_or(
        (SourceValue::Absent, SourceValue::Absent),
        |(value, layer)| {
            (
                SourceValue::Present(value),
                SourceValue::Present(json!(layer)),
            )
        },
    );
    json!({"value": source(value), "layer": source(layer)})
}

fn orders() -> Value {
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

pub(super) fn manifests() -> Value {
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
