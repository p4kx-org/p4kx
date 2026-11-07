// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::npc_spawns;

#[expect(
    clippy::too_many_lines,
    reason = "one sample of every kind of value an objective property holds"
)]
pub(super) fn values() -> serde_json::Value {
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
            "textId": "@mood",
            "value": "calm",
            "weighting": 0.25,
            "debugForceChooseThisOption": false,
            "dependentProperties": [null, {"property": null, "values": [null]}]
        }]},
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Count_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "integer", "options": [{
            "displayName": {"key": null, "en": null, "de": null},
            "textId": "@LOC_UNINITIALIZED",
            "value": 3,
            "variation": 1,
            "weighting": 0.25,
            "debugForceChooseThisOption": false,
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
        "value": {
            "type": "tags", "tags": ["Ship.Model.Mole"], "negativeTags": [],
            "sourceTags": {"tags": [null]}, "sourceNegativeTags": null, "options": []
        },
        "linkedOutput": null
    }, {
        "kind": "embedded",
        "variable": "Where_BP",
        "token": null,
        "contractProperty": null,
        "value": {"type": "location", "queryGuid": "e2000000-0000-4000-8000-000000000001"},
        "linkedOutput": null
    },
    npc_spawns::embedded_property("Crew_BP", &json!({
        "type": "npcSpawnDescriptions",
        "poolGuid": "e5000000-0000-4000-8000-000000000001"
    })),
    npc_spawns::embedded_property("Loot_BP", &json!({
        "type": "entitySpawnDescriptions",
        "poolGuid": "e6000000-0000-4000-8000-000000000001"
    })),
    npc_spawns::embedded_property("Giver_BP", &json!({
        "type": "organization",
        "conditions": [{
            "condition": "specificOrganizations",
            "organizationGuids": ["e7000000-0000-4000-8000-000000000001"]
        }],
        "forcedOrganizationGuid": null
    })), {
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
