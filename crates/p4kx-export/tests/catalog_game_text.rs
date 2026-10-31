// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exactly the game-text fields of the catalog carry `x-gameText` in its JSON Schema.

use serde_json::json;

const GAME_TEXT: [&str; 16] = [
    "description",
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
];

fn collect_properties<'s>(
    node: &'s serde_json::Value,
    found: &mut Vec<(&'s str, &'s serde_json::Value)>,
) {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(properties) = map.get("properties").and_then(serde_json::Value::as_object) {
                found.extend(
                    properties
                        .iter()
                        .map(|(name, value)| (name.as_str(), value)),
                );
            }
            for value in map.values() {
                collect_properties(value, found);
            }
        }
        serde_json::Value::Array(items) => {
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
        assert_eq!(carries, GAME_TEXT.contains(&name), "x-gameText on {name}");
        marked += usize::from(carries);
    }
    assert_eq!(
        marked, 27,
        "four descriptions, two display types, the contract title and description, mission type, contractor, faction name, pickup place, the process name, the deposit name, the names of a faction, a scope, a standing and a mission type, the variant of a location template, the title of a contract template, the three texts of an objective display, the two overrides of the mission details, the HUD text of an instance handler and the name of a marker"
    );
    let mut starmap_properties = Vec::new();
    collect_properties(&starmap, &mut starmap_properties);
    for (name, value) in starmap_properties {
        let carries = value.get("x-gameText") == Some(&json!(true));
        assert_eq!(carries, name == "name", "x-gameText on the starmap {name}");
    }
}
