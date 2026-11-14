// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The blueprint pools and the crafting tiers.

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

const INPUT_REF: &str = "synthetic-game2";
const BLUEPRINT_GUID: &str = "11000000-0000-4000-8000-000000000001";
const RESOURCE_GUID: &str = "b1000000-0000-4000-8000-000000000001";

pub(super) fn pools() -> Value {
    json!([{
        "guid": "e4000000-0000-4000-8000-000000000001",
        "className": "BP_REWARDS_Haul",
        "recordPath": "libs/foundry/records/crafting/blueprintrewards/bp_rewards_haul",
        "totalWeight": 4.0,
        "entries": [{
            "blueprintGuid": "11000000-0000-4000-8000-000000000001",
            "weight": 1.0
        }],
        "rewardSources": [{
            "recordGuid": "e5000000-0000-4000-8000-000000000001",
            "recordPath": "libs/foundry/records/contracts/contractgenerator/haul",
            "contractId": null, "contractName": "Haul_Intro", "chance": 0.5
        }]
    }])
}

pub(super) fn definitions() -> Value {
    json!({"availability": "available", "rows": [{
        "inputRef": INPUT_REF,
        "guid": present(json!(BLUEPRINT_GUID)),
        "recordName": present(json!("Blueprint.BP_CRAFT_core")),
        "fileName": present(json!("libs/foundry/records/crafting/bp_craft_core.xml")),
        "metadataTag": source(SourceValue::Absent),
        "className": present(json!("BP_CRAFT_core")),
        "recordPath": present(json!("libs/foundry/records/crafting/bp_craft_core")),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("Blueprint")),
        "source": {
            "roots": [field("tiers", "strongPointer", true, "0")],
            "values": [
                node(json!({"type": "array", "elements": [{"valueIndex": "1"}]})),
                node(json!({
                    "type": "struct", "sourceType": present(json!("BlueprintTier")),
                    "fields": [
                        field("craftTimeSeconds", "int32", false, "2"),
                        field("mandatoryCosts", "strongPointer", false, "3")
                    ]
                })),
                node(json!({"type": "number", "value": {"type": "int32", "value": 93784}})),
                node(json!({
                    "type": "struct", "sourceType": present(json!("BlueprintCost_Resource")),
                    "fields": []
                }))
            ]
        }
    }]})
}

fn field(name: &str, data_type: &str, is_array: bool, value_index: &str) -> Value {
    json!({
        "name": present(json!(name)),
        "declaration": {
            "dataType": present(json!(data_type)),
            "isArray": present(json!(is_array)),
            "target": present(json!(0))
        },
        "value": {"valueIndex": value_index}
    })
}

fn node(value: Value) -> Value {
    json!({"value": present(value), "originalError": null})
}

fn owner() -> Value {
    json!({"inputRef": INPUT_REF, "recordIndex": 0})
}

pub(super) fn tiers() -> Value {
    json!({"availability": "available", "rows": [{
        "blueprint": owner(),
        "source": {"valueIndex": "1"},
        "craftTimeSeconds": present(json!("93784")),
        "mandatory": {"valueIndex": "3"},
        "optional": [],
        "costs": [{
            "source": {"valueIndex": "3"},
            "context": {"source": null, "quantityMultiplier": null,
                        "compositionInclusion": null, "modifiers": []},
            "meaning": {"type": "resource", "resourceGuid": present(json!(RESOURCE_GUID)),
                        "quantityScu": present(json!(0.5)), "minQuality": present(json!("0"))}
        }],
        "research": {"source": null, "envelope": source(SourceValue::Absent)}
    }]})
}

pub(super) fn presentation() -> Value {
    json!({"availability": "available", "rows": [{
        "blueprint": owner(),
        "name": source(SourceValue::Null),
        "category": present(json!("d1000000-0000-4000-8000-000000000001")),
        "producedItem": source(SourceValue::Null),
        "firstTier": {"valueIndex": "1"},
        "ingredients": [{"cost": {"valueIndex": "3"}, "slot": null}],
        "defaultAvailable": present(json!(false)),
        "dismantle": present(json!({
            "processGuid": "f1000000-0000-4000-8000-000000000001", "tier": {"valueIndex": "1"}
        }))
    }]})
}
