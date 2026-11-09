// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exactly the game-text fields of the catalog carry `x-gameText` in its JSON Schema.

use std::collections::BTreeSet;

use serde_json::{Value, json};

const GAME_TEXT: [&str; 28] = [
    "description",
    "displayDescription",
    "gameTexts",
    "displayName",
    "displayType",
    "title",
    "missionType",
    "typeName",
    "contractor",
    "place",
    "processName",
    "depositName",
    "markerLabel",
    "shortDescription",
    "longDescription",
    "titleOverride",
    "descriptionOverride",
    "hudObjectiveText",
    "hudTitle",
    "missionGiver",
    "commsChannelName",
    "endReason",
    "givenName",
    "givenLastName",
    "givenNickName",
    "objectiveLongDescription",
    "objectiveShortDescription",
    "causeOfDeath",
];

fn collect_properties<'s>(node: &'s Value, found: &mut Vec<(&'s str, &'s Value)>) {
    match node {
        Value::Object(map) => {
            if let Some(properties) = map.get("properties").and_then(Value::as_object) {
                found.extend(
                    properties
                        .iter()
                        .map(|(name, value)| (name.as_str(), value)),
                );
                for value in properties.values() {
                    collect_properties(value, found);
                }
            }
            for (name, value) in map {
                if name != "properties" {
                    collect_properties(value, found);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_properties(value, found);
            }
        }
        _ => {}
    }
}

#[test]
fn exactly_the_game_text_fields_carry_the_game_text_annotation() {
    let mut schema = p4kx_export::catalog_schema();
    let starmap = schema["$defs"]
        .as_object_mut()
        .and_then(|defs| defs.remove("StarmapObject"))
        .expect("the starmap object definition");
    let mut properties = Vec::new();
    collect_properties(&schema, &mut properties);
    let mut marked = 0;
    for (name, value) in properties {
        let carries = value.get("x-gameText") == Some(&json!(true));
        assert_eq!(
            carries,
            GAME_TEXT.contains(&name) && !is_only_public_name(&schema, value),
            "x-gameText on {name}: {value}"
        );
        marked += usize::from(carries);
    }
    assert_eq!(
        marked, 51,
        "the expected resource, mission, name and option game-text fields"
    );
    let mut starmap_properties = Vec::new();
    collect_properties(&starmap, &mut starmap_properties);
    for (name, value) in starmap_properties {
        let carries = value.get("x-gameText") == Some(&json!(true));
        assert_eq!(carries, name == "name", "x-gameText on the starmap {name}");
    }
}

fn is_only_public_name(schema: &Value, field: &Value) -> bool {
    references(field, schema, "#/$defs/PublicNameRef", &mut BTreeSet::new())
        && !references(field, schema, "#/$defs/GameTextRef", &mut BTreeSet::new())
}

fn references<'schema>(
    node: &'schema Value,
    schema: &'schema Value,
    target: &str,
    seen: &mut BTreeSet<&'schema str>,
) -> bool {
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        if reference == target {
            return true;
        }
        if seen.insert(reference)
            && let Some(pointer) = reference.strip_prefix('#')
            && let Some(referred) = schema.pointer(pointer)
            && references(referred, schema, target, seen)
        {
            return true;
        }
    }
    match node {
        Value::Object(fields) => fields
            .values()
            .any(|value| references(value, schema, target, seen)),
        Value::Array(values) => values
            .iter()
            .any(|value| references(value, schema, target, seen)),
        _ => false,
    }
}

#[test]
fn manufacturer_localization_uses_separate_public_and_protected_text_domains() {
    let schema = p4kx_export::catalog_schema();
    let localization = &schema["$defs"]["ManufacturerLocalization"];
    let properties = &localization["properties"];
    assert!(is_only_public_name(&schema, &properties["name"]));
    assert!(references(
        &properties["description"],
        &schema,
        "#/$defs/GameTextRef",
        &mut BTreeSet::new()
    ));
    assert_eq!(properties["description"]["x-gameText"], true);
    assert!(properties["name"].get("x-gameText").is_none());
    for field in ["sourceType", "name", "description"] {
        assert!(
            localization["required"]
                .as_array()
                .expect("required localization fields")
                .iter()
                .any(|required| required == field)
        );
    }
}
