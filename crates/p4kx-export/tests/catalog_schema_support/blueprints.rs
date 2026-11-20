// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The blueprint pools and the crafting tiers.

use p4kx_extraction::SourceValue;
use serde_json::{Map, Value, json};

use super::source_values::{present, source};

const INPUT_REF: &str = "synthetic-game2";
const BLUEPRINT_GUID: &str = "11000000-0000-4000-8000-000000000001";
const RESOURCE_GUID: &str = "b1000000-0000-4000-8000-000000000001";

const POOL_GUID: &str = "e4000000-0000-4000-8000-000000000001";
const POOL_PATH: &str = "libs/foundry/records/crafting/blueprintrewards/bp_rewards_haul";
const POOL_INDEX: u32 = 2;

pub(super) fn pools() -> Value {
    let mut row = record_header(
        POOL_GUID,
        POOL_PATH,
        POOL_INDEX,
        "BlueprintPoolRecord",
        "BP_REWARDS_Haul",
    );
    row.insert(
        "blueprintRewards".to_owned(),
        present(json!([present(json!({
            "sourceType": present(json!("BlueprintReward")),
            "blueprintRecord": record_reference(BLUEPRINT_GUID, "libs/foundry/records/crafting/bp_craft_core", 0, "Blueprint"),
            "weight": present(json!({"type": "f32", "rawBits": "3f800000", "finite": 1.0})),
            "additionalFields": empty_arena()
        }))])),
    );
    row.insert("additionalFields".to_owned(), empty_arena());
    table(Value::Object(row))
}

pub(super) fn pool_weights() -> Value {
    table(json!({
        "pool": {"inputRef": INPUT_REF, "guid": present(json!(POOL_GUID)), "recordIndex": present(json!(POOL_INDEX))},
        "totalWeight": present(json!({"rawBits": "3f800000", "finite": 1.0}))
    }))
}

pub(super) fn reward_sources() -> Value {
    let mut row = record_header(
        "e5000000-0000-4000-8000-000000000001",
        "libs/foundry/records/contracts/contractgenerator/haul",
        3,
        "ContractGenerator",
        "Haul",
    );
    row.insert("sourceOrdinal".to_owned(), present(json!("0")));
    row.insert("poolStorage".to_owned(), present(json!("reference")));
    row.insert(
        "pool".to_owned(),
        record_reference(POOL_GUID, POOL_PATH, POOL_INDEX, "BlueprintPoolRecord"),
    );
    row.insert("contractId".to_owned(), source(SourceValue::Absent));
    row.insert("contractName".to_owned(), present(json!("Haul_Intro")));
    row.insert(
        "chance".to_owned(),
        present(json!({"type": "f32", "rawBits": "3f000000", "finite": 0.5})),
    );
    table(Value::Object(row))
}

fn record_header(
    guid: &str,
    path: &str,
    index: u32,
    field_type: &str,
    class_name: &str,
) -> Map<String, Value> {
    [
        ("guid", present(json!(guid))),
        (
            "recordName",
            present(json!(format!("{field_type}.{class_name}"))),
        ),
        ("className", present(json!(class_name))),
        ("recordPath", present(json!(path))),
        ("recordIndex", present(json!(index))),
        ("sourceType", present(json!(field_type))),
        ("fileName", present(json!(format!("{path}.xml")))),
        ("metadataTag", source(SourceValue::Absent)),
    ]
    .into_iter()
    .map(|(key, cell)| (key.to_owned(), cell))
    .collect()
}

fn record_reference(guid: &str, path: &str, index: u32, target: &str) -> Value {
    present(json!({
        "guid": present(json!(guid)),
        "recordPath": present(json!(path)),
        "targetRecordIndex": present(json!(index)),
        "targetType": present(json!(target)),
        "status": "present",
    }))
}

fn table(row: Value) -> Value {
    Value::Object(Map::from_iter([
        ("availability".to_owned(), json!("available")),
        ("rows".to_owned(), Value::Array(vec![row])),
    ]))
}

fn empty_arena() -> Value {
    json!({"roots": [], "values": []})
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
        "tierIndex": present(json!(0)),
        "mandatory": {"valueIndex": "3"},
        "optional": [],
        "costs": [{
            "position": present(json!({"type": "mandatory"})),
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

pub(super) fn dismantle_processes() -> Value {
    let empty = || json!({"roots": [], "values": []});
    let zero = || present(json!({"type": "int32", "value": 0}));
    let time = present(json!({
        "sourceType": present(json!("TimeValue_Partitioned")),
        "days": zero(),
        "hours": zero(),
        "minutes": zero(),
        "seconds": present(json!({"type": "f32", "rawBits": "416c0000", "finite": 14.75})),
        "authoredFields": empty()
    }));
    let process = present(json!({
        "sourceType": present(json!("GenericCraftingProcess_Dismantle")),
        "efficiency": present(json!({"type": "f32", "rawBits": "3f000000", "finite": 0.5})),
        "dismantleTime": time,
        "timeSeconds": present(json!("15")),
        "authoredFields": empty()
    }));
    let blueprint = present(json!({
        "sourceType": present(json!("GenericCraftingBlueprint")),
        "processSpecificData": process,
        "authoredFields": empty()
    }));
    json!({"availability": "available", "rows": [{
        "guid": present(json!("f1000000-0000-4000-8000-000000000001")),
        "recordName": present(json!("CraftingBlueprintRecord.GlobalGenericDismantle")),
        "className": present(json!("GlobalGenericDismantle")),
        "recordPath": present(json!("libs/foundry/records/crafting/blueprints/dismantle/globalgenericdismantle")),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("CraftingBlueprintRecord")),
        "fileName": present(json!("libs/foundry/records/crafting/blueprints/dismantle/globalgenericdismantle.xml")),
        "metadataTag": source(SourceValue::Absent),
        "blueprint": blueprint,
        "authoredFields": empty()
    }]})
}

pub(super) fn globals() -> Value {
    json!({"availability": "available", "rows": [{
        "guid": present(json!("12000000-0000-4000-8000-000000000002")),
        "recordName": present(json!("CraftingGlobalParams.Synthetic")),
        "className": present(json!("Synthetic")),
        "recordPath": present(json!("libs/foundry/records/crafting/globalparams/synthetic")),
        "recordIndex": present(json!(1)),
        "sourceType": present(json!("CraftingGlobalParams")),
        "fileName": present(json!("libs/foundry/records/crafting/globalparams/synthetic.xml")),
        "metadataTag": source(SourceValue::Absent),
        "defaultBlueprintSelection": source(SourceValue::Absent),
        "dismantleBlacklistResources": present(json!([])),
        "dismantleBlacklistEntityClasses": present(json!([])),
        "additionalFields": {"roots": [], "values": []}
    }]})
}
