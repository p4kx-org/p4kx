// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use crate::commodity_v3_fixture::{self as fixture, TestResult};
use crate::commodity_v3_source_inventory::{body, header};

use crate::commodity_v3_source_inventory as inventory;
pub(crate) use crate::commodity_v3_source_inventory::uses;

#[test]
fn definition_headers_keep_the_actual_physical_source_owner() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::definition(&catalog, fixture::ALPHA)?;
    assert_eq!(row["recordIndex"]["value"], fixture::ALPHA_INDEX);
    assert_eq!(row["sourceType"]["value"], "EntityClassDefinition");
    assert_eq!(row["className"]["value"], "Alpha");
    assert_eq!(
        row["recordPath"]["value"],
        "libs/foundry/records/entities/commodities/Alpha"
    );
    assert_eq!(
        uses(&catalog, &row["guid"])?,
        BTreeSet::from([(fixture::ALPHA_INDEX, header("guid"), "payload".to_owned())])
    );
    assert_eq!(
        uses(&catalog, &row["className"])?,
        BTreeSet::from([
            (
                fixture::ALPHA_INDEX,
                header("recordName"),
                "conversion".to_owned()
            ),
            (
                fixture::ALPHA_INDEX,
                header("sourceType"),
                "selector".to_owned()
            ),
        ]),
        "class-name normalization retains its actual header inputs"
    );
    for (field, path, role) in [
        ("recordPath", "fileName", "conversion"),
        ("recordIndex", "recordIndex", "selector"),
        ("sourceType", "sourceType", "selector"),
    ] {
        assert_eq!(
            uses(&catalog, &row[field])?,
            BTreeSet::from([(fixture::ALPHA_INDEX, header(path), role.to_owned())])
        );
    }
    Ok(())
}

#[test]
fn ordered_matching_bindings_keep_original_positions_and_nullable_composition_slots() -> TestResult
{
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::definition(&catalog, fixture::ALPHA)?;
    let authoring = fixture::payload(row, "entity")?;
    assert_eq!(positions(authoring, "purchasableParameters")?, vec![2, 4]);
    assert_eq!(positions(authoring, "displayParameters")?, vec![1, 2]);
    assert_eq!(positions(authoring, "resourceContainers")?, vec![3, 5]);
    let containers = fixture::payload(authoring, "resourceContainers")?
        .as_array()
        .ok_or("containers")?;
    let first = containers.first().ok_or("first container")?;
    let slots = first["value"]["defaultComposition"]["value"]
        .as_array()
        .ok_or("composition slots")?;
    assert_eq!(
        slots.len(),
        4,
        "null and repeated reference slots retain original positions"
    );
    assert_eq!(slots.first().ok_or("null slot")?["state"], "null");
    assert_eq!(
        slots.get(1).ok_or("null reference")?["value"]["entry"]["state"],
        "null"
    );
    assert_eq!(
        slots.get(2).ok_or("resource reference")?["value"]["entry"]["value"]["guid"]["value"],
        fixture::guid(fixture::RESOURCE)
    );
    assert_eq!(
        slots.get(3).ok_or("second reference")?["value"]["entry"]["value"]["guid"]["value"],
        fixture::guid(fixture::RESOURCE_FALLBACK)
    );
    Ok(())
}

#[test]
fn aggregate_child_inputs_and_each_nested_carrier_have_complete_reference_closure() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let row = fixture::definition(&catalog, fixture::ALPHA)?;
    let aggregate = uses(&catalog, &row["entity"])?;
    assert!(aggregate.contains(&(
        fixture::ALPHA_INDEX,
        body("Components[2].displayName"),
        "payload".to_owned()
    )));
    assert!(
        aggregate.contains(&(1, header("recordIndex"), "selector".to_owned())),
        "retained reference target headers contribute to the aggregate independently"
    );
    for definition in catalog["definitions"]["commodities"]["rows"]
        .as_array()
        .ok_or("definitions")?
    {
        carriers(definition)?;
    }
    for presentation in catalog["projections"]["commodityPresentation"]["rows"]
        .as_array()
        .ok_or("presentations")?
    {
        carriers(presentation)?;
    }
    inventory::reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn absent_null_empty_and_explicit_empty_key_are_separate_source_states() -> TestResult {
    for (declared, state) in [(false, "absent"), (true, "null")] {
        let (_, catalog) = fixture::build(&fixture::parent_arrays(declared))?;
        let authoring = fixture::payload(fixture::definition(&catalog, fixture::ALPHA)?, "entity")?;
        for field in [
            "purchasableParameters",
            "displayParameters",
            "resourceContainers",
        ] {
            assert_eq!(
                authoring[field]["state"], state,
                "the nearest actual parent state survives"
            );
            assert!(authoring[field]["value"].is_null());
        }
    }
    let (_, catalog) = fixture::build(&fixture::database())?;
    let definition = fixture::definition(&catalog, fixture::UNLINKED)?;
    let authoring = fixture::payload(definition, "entity")?;
    for field in ["displayParameters", "resourceContainers"] {
        assert_eq!(authoring[field]["state"], "present");
        assert_eq!(
            authoring[field]["value"].as_array().map(Vec::len),
            Some(0),
            "empty is a present collection"
        );
    }
    let key_ref = &authoring["purchasableParameters"]["value"]
        .as_array()
        .ok_or("purchase")?
        .first()
        .ok_or("first purchase")?["value"]["displayName"]["value"];
    assert_eq!(
        fixture::text_row(&catalog, "publicNames", key_ref)?["key"]["value"],
        "",
        "an explicit empty Locale remains an authored key"
    );
    let presentation = fixture::presentation(&catalog, fixture::UNLINKED)?;
    for field in ["name", "description", "displayType"] {
        assert_eq!(presentation[field]["state"], "present");
        assert!(
            presentation[field]["value"].is_null(),
            "the presentation did not invent a key"
        );
    }
    Ok(())
}

#[test]
fn the_first_declared_dangling_resource_is_not_replaced_by_a_later_readable_link() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::dangling_first())?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    assert_eq!(
        row["subtypeGuid"]["value"],
        fixture::guid(fixture::DANGLING_RESOURCE),
        "the first non-null authored reference wins independently of target readability"
    );
    let authoring = fixture::payload(fixture::definition(&catalog, fixture::ALPHA)?, "entity")?;
    let container = authoring["resourceContainers"]["value"]
        .as_array()
        .ok_or("containers")?
        .first()
        .ok_or("first container")?;
    let composition = container["value"]["defaultComposition"]["value"]
        .as_array()
        .ok_or("composition")?;
    let first = &composition.first().ok_or("first composition")?["value"]["entry"]["value"];
    assert_eq!(first["status"], "dangling");
    assert_eq!(
        first["guid"]["value"],
        fixture::guid(fixture::DANGLING_RESOURCE)
    );
    for field in ["targetRecordIndex", "recordPath", "targetType"] {
        assert_eq!(
            first[field]["state"], "absent",
            "unresolved target headers are not fabricated"
        );
    }
    assert_eq!(
        composition.get(1).ok_or("readable neighbor")?["value"]["entry"]["value"]["status"],
        "present",
        "the later readable link still survives in the source definition"
    );
    Ok(())
}

#[test]
fn successful_choices_do_not_promote_unknown_dcb_lineage() -> TestResult {
    let catalog = fixture::unknown(&fixture::database())?;
    let row = fixture::presentation(&catalog, fixture::ALPHA)?;
    for field in [
        "commodityGuid",
        "name",
        "description",
        "displayType",
        "subtypeGuid",
    ] {
        assert!(
            row[field]["containsEncryptedSource"].is_null(),
            "actual unknown source evidence stays unknown"
        );
        assert_eq!(
            row[field]["sourceSetComplete"], true,
            "contributor completeness is separate from known encryption"
        );
    }
    Ok(())
}

fn positions(authoring: &Value, field: &str) -> TestResult<Vec<u64>> {
    fixture::payload(authoring, field)?
        .as_array()
        .ok_or("ordered bindings")?
        .iter()
        .map(|row| {
            fixture::at(row, "/value/sourcePosition/value")?
                .as_u64()
                .ok_or_else(|| "binding source position is missing".into())
        })
        .collect()
}

fn carriers(value: &Value) -> TestResult {
    match value {
        Value::Object(object) if object.contains_key("state") => {
            for name in [
                "state",
                "value",
                "containsEncryptedSource",
                "sourceUsesRefs",
                "sourceSetComplete",
            ] {
                assert!(
                    object.contains_key(name),
                    "every immediate nullable carrier key is present"
                );
            }
            carriers(object.get("value").ok_or("value key")?)?;
        }
        Value::Object(object) => {
            for (name, value) in object {
                if matches!(
                    name.as_str(),
                    "status" | "type" | "code" | "selectionDiagnostics"
                ) {
                    continue;
                }
                assert!(
                    value.get("state").is_some(),
                    "nested source-backed facts have their own immediate carrier"
                );
                carriers(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                assert!(
                    value.get("state").is_some(),
                    "authored array members are independently sourced"
                );
                carriers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
