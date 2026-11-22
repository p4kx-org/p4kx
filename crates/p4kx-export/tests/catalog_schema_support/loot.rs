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
        "weightSource": values::present(json!({"valueIndex": "1"})),
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
        "recordIndex": 1,
        "entries": [{
            "name": "Ammo",
            "weight": 10.0,
            "weightSource": values::present(json!({"valueIndex": "1"})),
            "archetypeGuid": null,
            "archetype": [archetype_entry()],
            "priorityCount": 1,
            "choiceLimit": 2,
            "dupeLimit": 3,
            "dupeLimitSource": values::present(json!({"valueIndex": "2"})),
            "chanceToExist": 0.75
        }, {
            "name": "Medpens",
            "weight": 1.0,
            "weightSource": values::present(json!({"valueIndex": "1"})),
            "archetypeGuid": "a9000000-0000-4000-8000-000000000002",
            "archetype": null,
            "priorityCount": null,
            "choiceLimit": null,
            "dupeLimit": null,
            "dupeLimitSource": values::source(SourceValue::Absent),
            "chanceToExist": null
        }]
    }])
}

pub(super) fn archetypes() -> Value {
    json!([{
        "guid": "a9000000-0000-4000-8000-000000000002",
        "className": "V3LootArchetype_Medpen_Healing",
        "recordPath": "libs/foundry/records/lootgeneration/lootarchetypes/v3lootarchetype_medpen_healing",
        "recordIndex": 2,
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

fn legacy_header(
    guid: &str,
    class: &str,
    path: &str,
    kind: &str,
    index: u32,
) -> serde_json::Map<String, Value> {
    let header = json!({
        "guid":values::present(json!(guid)),
        "recordName":values::present(json!(format!("{kind}.{class}"))),
        "className":values::present(json!(class)),
        "recordPath":values::present(json!(path)),
        "recordIndex":values::present(json!(index)),
        "sourceType":values::present(json!(kind)),
        "fileName":values::present(json!(format!("{path}.xml"))),
        "metadataTag":values::source(SourceValue::Absent),
        "fields":[],
        "values":[]
    });
    if let Value::Object(header) = header {
        header
    } else {
        serde_json::Map::new()
    }
}

fn legacy_rows(cells: impl IntoIterator<Item = Value>) -> Value {
    let rows: Vec<_> = cells.into_iter().map(values::present).collect();
    values::present(json!(rows))
}

fn legacy_number(number: i32) -> Value {
    values::present(json!({"type":"int32","value":number}))
}

pub(super) fn legacy_tables() -> Value {
    let mut row = legacy_header(
        "a9000000-0000-4000-8000-000000000005",
        "Ammo",
        "libs/foundry/records/lootgeneration/loottables/ammo",
        "LootTable",
        3,
    );
    row.insert(
        "entries".into(),
        legacy_rows([
            json!({
                "weight":legacy_number(1),
                "archetypeGuid":values::present(json!("a9000000-0000-4000-8000-000000000006")),
                "minResults":legacy_number(0),
                "maxResults":legacy_number(2)
            }),
            json!({
                "weight":values::source(SourceValue::Null),
                "archetypeGuid":values::source(SourceValue::Unreadable),
                "minResults":values::source(SourceValue::Absent),
                "maxResults":values::source(SourceValue::Invalid)
            }),
        ]),
    );
    json!([row])
}

pub(super) fn legacy_archetypes() -> Value {
    let mut row = legacy_header(
        "a9000000-0000-4000-8000-000000000006",
        "LootArchetype_AmmoCache_Common",
        "libs/foundry/records/lootgeneration/lootarchetypes/lootarchetype_ammocache_common",
        "LootArchetype",
        4,
    );
    let tag = "a9000000-0000-4000-8000-000000000010";
    row.insert("excludedTagGuids".into(), legacy_rows([json!(tag)]));
    row.insert(
        "entries".into(),
        legacy_rows([json!({
            "name":values::present(json!("Ammo")),
            "weight":legacy_number(1),
            "tagGuid":values::present(json!(tag)),
            "additionalPositiveTagGuids":legacy_rows([json!(tag)]),
            "additionalNegativeTagGuids":legacy_rows([]),
            "stackSize":values::present(json!({"min":legacy_number(3),"max":legacy_number(3)})),
            "spawnWith":legacy_rows([legacy_spawn(tag)])
        })]),
    );
    row.insert("secondaryGroups".into(), legacy_rows([json!({
        "groupName":values::present(json!("Rarity")),
        "entries":legacy_rows([json!({"weight":legacy_number(1),"tagGuid":values::present(json!(tag))})])
    })]));
    json!([row])
}

fn legacy_spawn(tag: &str) -> Value {
    json!({
        "name":values::present(json!("Suitable ammunition")),
        "mode":values::present(json!("MostSimilar")),
        "min":legacy_number(1),
        "max":legacy_number(3),
        "positiveTagGuids":legacy_rows([json!(tag)]),
        "negativeTagGuids":legacy_rows([])
    })
}

pub(super) fn containers() -> Value {
    json!([{
        "multiConfigGuid": values::present(json!("a7000000-0000-4000-8000-000000000001")),
        "container": values::present(json!("Data\\ObjectContainers\\PU\\loc\\mod\\drlct\\loot_area.socpak")),
        "system": values::present(json!("stanton")),
        "starmapObjectGuid": "a2000000-0000-4000-8000-000000000001",
        "entityCount": values::present(json!(13)),
        "tagCounts": values::present(json!([{
            "tagGuid": values::present(json!("a7000000-0000-4000-8000-000000000002")),
            "entities": values::present(json!(4)),
        }]))
    }, {
        "multiConfigGuid": values::present(json!("a7000000-0000-4000-8000-000000000001")),
        "container": values::present(json!("ObjectContainers/PU/loc/missing.socpak")),
        "system": values::present(json!("pyro")),
        "starmapObjectGuid": null,
        "entityCount": values::source(SourceValue::Absent),
        "tagCounts": values::source(SourceValue::Absent)
    }])
}

const KINDS: [&str; 6] = [
    "lootSecondaryChoiceSingleLayers",
    "lootSecondaryChoiceMultiLayers",
    "lootThemes",
    "hierarchicalContextualDataPresets",
    "lootGenerationGlobalParams",
    "subHarvestableConfigs",
];

pub(crate) fn add_definition_tables(catalog: &mut Value) {
    let Some(completeness) = catalog
        .get_mut("completeness")
        .and_then(Value::as_object_mut)
    else {
        unreachable!("fixture completeness object");
    };
    for kind in KINDS {
        completeness.insert(kind.to_owned(), json!({"status": "complete"}));
    }
    let Some(definitions) = catalog
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
    else {
        unreachable!("fixture definitions object");
    };
    for kind in KINDS.into_iter().chain(["subHarvestableMultiConfigs"]) {
        definitions.insert(kind.to_owned(), super::absent_table());
    }
}
