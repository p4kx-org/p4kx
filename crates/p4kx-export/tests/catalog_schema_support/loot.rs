// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each loot kind, with every field the schema lists.

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values as values;

fn selector() -> Value {
    json!({
        "selectorType": "LootArchetypeV3Selector_Tags",
        "positiveTagGuids": ["a9000000-0000-4000-8000-000000000010"],
        "negativeTagGuids": ["a9000000-0000-4000-8000-000000000011"],
        "entityClassGuids": ["a9000000-0000-4000-8000-000000000012"]
    })
}

fn archetype_entry() -> Value {
    json!({
        "name": "Ammo",
        "weight": 3.0,
        "selector": selector(),
        "stackSize": {"min": 1.0, "max": 3.0},
        "spawnWith": [{
            "name": "Suitable ammo",
            "chance": 1.0,
            "mode": "MostSimilar",
            "amount": {"min": 2.0, "max": 4.0},
            "selector": selector()
        }]
    })
}

pub(super) fn tables() -> Value {
    json!([{
        "guid": "a9000000-0000-4000-8000-000000000001",
        "className": "V3LootTable_Actor_CQC",
        "recordPath": "libs/foundry/records/lootgeneration/loottables/v3loottable_actor_cqc",
        "entries": [{
            "name": "Ammo",
            "weight": 10.0,
            "archetypeGuid": null,
            "archetype": [archetype_entry()],
            "priorityCount": 1,
            "choiceLimit": 2,
            "dupeLimit": 3,
            "chanceToExist": 0.75
        }, {
            "name": "Medpens",
            "weight": 1.0,
            "archetypeGuid": "a9000000-0000-4000-8000-000000000002",
            "archetype": null,
            "priorityCount": null,
            "choiceLimit": null,
            "dupeLimit": null,
            "chanceToExist": null
        }]
    }])
}

pub(super) fn archetypes() -> Value {
    json!([{
        "guid": "a9000000-0000-4000-8000-000000000002",
        "className": "V3LootArchetype_Medpen_Healing",
        "recordPath": "libs/foundry/records/lootgeneration/lootarchetypes/v3lootarchetype_medpen_healing",
        "entries": [archetype_entry()]
    }])
}

/// Constructs complete schema data without claiming an actual original read.
pub(super) fn filters() -> Value {
    json!({"availability":"available","rows":[{
        "guid":values::present(json!("a9000000-0000-4000-8000-000000000003")),
        "recordName":values::present(json!("PoolFilterRecord.SyntheticLootFilter")),
        "className":values::present(json!("SyntheticLootFilter")),
        "recordPath":values::present(json!("libs/foundry/records/lootgeneration/filters/synthetic")),
        "recordIndex":values::present(json!(0)),
        "sourceType":values::present(json!("PoolFilterRecord")),
        "fileName":values::present(json!("libs/foundry/records/lootgeneration/filters/synthetic.xml")),
        "metadataTag":values::source(SourceValue::Absent),
        "fields":[filter_field("filter","strongPointer",false,0,0)],
        "values":filter_values()
    }]})
}

fn filter_values() -> Vec<Value> {
    vec![
        filter_value(SourceValue::Present(json!({"type":"struct",
            "sourceType":values::present(json!("PoolFilter_Sequence")),
            "fields":[filter_field("filters","class",true,1,1)]}))),
        filter_value(SourceValue::Present(
            json!({"type":"array","elements":[{"valueIndex":"2"},{"valueIndex":"7"}]}),
        )),
        filter_instance(3, 4, 5),
        filter_text("No faction"),
        filter_text("Cumulative"),
        filter_value(SourceValue::Present(json!({"type":"struct",
            "sourceType":values::present(json!("PoolFilter_RecordRef")),
            "fields":[filter_field("filterRecord","reference",false,0,6)]}))),
        filter_reference(),
        filter_instance(8, 9, 10),
        filter_text("Event"),
        filter_text("Additive"),
        filter_value(SourceValue::Null),
    ]
}

fn filter_instance(name: u64, mode: u64, filter: u64) -> Value {
    filter_value(SourceValue::Present(json!({"type":"struct",
        "sourceType":values::present(json!("PoolFilterInstance")),
        "fields":[filter_field("name","string",false,0,name),
            filter_field("mode","string",false,0,mode),
            filter_field("filter","strongPointer",false,0,filter)]})))
}

fn filter_field(name: &str, data_type: &str, is_array: bool, target: u16, index: u64) -> Value {
    json!({"name":values::present(json!(name)),"declaration":{
        "dataType":values::present(json!(data_type)),
        "isArray":values::present(json!(is_array)),
        "target":values::present(json!(target))},"value":{"valueIndex":index.to_string()}})
}

fn filter_text(text: &str) -> Value {
    filter_value(SourceValue::Present(
        json!({"type":"string","value":{"type":"public","sourceText":text}}),
    ))
}

fn filter_reference() -> Value {
    filter_value(SourceValue::Present(
        json!({"type":"recordReference","value":{
        "targetRecordIndex":values::present(json!(1)),
        "guid":values::present(json!("a9000000-0000-4000-8000-000000000004")),
        "recordPath":values::present(json!("libs/foundry/records/lootgeneration/filters/target")),
        "targetType":values::present(json!("PoolFilterRecord")),"status":"present"}}),
    ))
}

fn filter_value(value: SourceValue<Value>) -> Value {
    json!({"value":values::source(value),"originalError":null})
}

pub(super) fn legacy_tables() -> Value {
    json!([{
        "guid": "a9000000-0000-4000-8000-000000000005",
        "className": "Ammo",
        "recordPath": "libs/foundry/records/lootgeneration/loottables/ammo",
        "entries": [{
            "weight": 1.0,
            "archetypeGuid": "a9000000-0000-4000-8000-000000000006",
            "minResults": 0,
            "maxResults": 2
        }, {
            "weight": null,
            "archetypeGuid": null,
            "minResults": null,
            "maxResults": null
        }]
    }])
}

pub(super) fn legacy_archetypes() -> Value {
    json!([{
        "guid": "a9000000-0000-4000-8000-000000000006",
        "className": "LootArchetype_AmmoCache_Common",
        "recordPath": "libs/foundry/records/lootgeneration/lootarchetypes/lootarchetype_ammocache_common",
        "excludedTagGuids": ["a9000000-0000-4000-8000-000000000010"],
        "entries": [{
            "name": "Ammo",
            "weight": 1.0,
            "tagGuid": "a9000000-0000-4000-8000-000000000011",
            "additionalPositiveTagGuids": ["a9000000-0000-4000-8000-000000000012"],
            "additionalNegativeTagGuids": [],
            "stackSize": {"min": 3.0, "max": 3.0},
            "spawnWith": [{
                "name": "Suitable ammunition",
                "mode": "MostSimilar",
                "min": 1.0,
                "max": 3.0,
                "positiveTagGuids": ["a9000000-0000-4000-8000-000000000012"],
                "negativeTagGuids": []
            }]
        }],
        "secondaryGroups": [{
            "groupName": "Rarity",
            "entries": [{"weight": 1.0, "tagGuid": "a9000000-0000-4000-8000-000000000011"}]
        }]
    }])
}

pub(super) fn containers() -> Value {
    json!([{
        "multiConfigGuid": "a7000000-0000-4000-8000-000000000001",
        "container": "Data\\ObjectContainers\\PU\\loc\\mod\\drlct\\loot_area.socpak",
        "system": "stanton",
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "entityCount": 13,
        "tagCounts": [{"tagGuid": "a7000000-0000-4000-8000-000000000002", "entities": 4}]
    }, {
        "multiConfigGuid": "a7000000-0000-4000-8000-000000000001",
        "container": "ObjectContainers/PU/loc/missing.socpak",
        "system": "pyro",
        "starmapObjectGuid": null,
        "entityCount": null,
        "tagCounts": []
    }])
}
