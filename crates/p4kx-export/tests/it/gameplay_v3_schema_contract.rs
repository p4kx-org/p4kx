// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_testkit::TestValue;
use serde_json::{Value, json};

const REQUIRED_CELLS: &[&str] = &[
    "/guid",
    "/recordName",
    "/className",
    "/recordPath",
    "/recordIndex",
    "/sourceType",
    "/fileName",
    "/metadataTag",
    "/propertyName",
    "/unitFormat",
    "/displayTransformation",
    "/displayTransformation/value/nodes/0/sourceType",
    "/displayTransformation/value/nodes/0/payload",
    "/displayTransformation/value/nodes/0/payload/value/transformations",
    "/displayTransformation/value/nodes/0/payload/value/transformations/value/1",
    "/nameOverrides",
    "/nameOverrides/value/0",
    "/nameOverrides/value/0/value/sourceType",
    "/nameOverrides/value/0/value/payload/value/propertyName",
    "/nameOverrides/value/0/value/payload/value/condition",
    "/nameOverrides/value/0/value/payload/value/condition/value/sourceType",
    "/nameOverrides/value/0/value/payload/value/condition/value/payload",
    "/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes",
    "/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchSubTypes",
    "/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchSubTypes/value/0",
    "/displayTransformation/value/nodes/2/sourceType",
    "/displayTransformation/value/nodes/2/payload",
    "/displayTransformation/value/nodes/2/payload/value/transformations/value/0",
    "/displayTransformation/value/nodes/3/sourceType",
    "/displayTransformation/value/nodes/3/payload",
    "/displayTransformation/value/nodes/3/payload/value/scale",
    "/nameOverrides/value/0/value/payload",
    "/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes/value/0",
];

#[test]
fn gameplay_v3_schema_requires_recursive_source_cells_closed_payload_tags_and_open_game_labels()
-> TestResult {
    let database = schema::schema().record(
        values::record(values::FIRST, "Primary", values::recursive()).value(
            "nameOverrides",
            TestValue::Array(vec![values::name_override(
                "@resource_name",
                values::item_type(
                    vec![TestValue::text("SyntheticA")],
                    vec![TestValue::text("SyntheticSub")],
                ),
            )]),
        ),
    );
    source::declarations(&database)?;
    source::with_tree(&database, 0, |tree| {
        assert_eq!(
            source::structure(tree.root().field("displayTransformation"))?.type_name(),
            schema::SEQUENCE,
            "the fixture's actual pointer provides a recursive source"
        );
        assert_eq!(
            source::array(tree.root().field("nameOverrides"))?.len(),
            1,
            "one actual inline class occurrence"
        );
        Ok(())
    })?;
    let (_, catalog) = support::normal(&database)?;
    support::row(&catalog, 0)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&catalog),
        "the actual source-backed producer satisfies its schema"
    );
    required_cells(&validator, &catalog)?;
    structural_controls(&validator, &catalog)?;
    open_labels_closed_controls(&validator, &catalog)?;
    compound_annotations(&schema)
}

fn required_cells(validator: &jsonschema::Validator, catalog: &Value) -> TestResult {
    for relative in REQUIRED_CELLS {
        required_cell(validator, catalog, relative)?;
    }
    let mut missing = catalog.clone();
    missing
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions")?
        .remove("gameplayProperties")
        .ok_or("required D table")?;
    assert!(
        !validator.is_valid(&missing),
        "the canonical definition table is required"
    );
    Ok(())
}

fn compound_annotations(schema: &Value) -> TestResult {
    let definitions = schema
        .get("$defs")
        .and_then(Value::as_object)
        .ok_or("schema definitions")?;
    let mut compounds = 0;
    for (name, definition) in definitions {
        if (name.contains("DisplayTransformation") || name.contains("PropertyNameOverride"))
            && (definition.get("properties").is_some()
                || definition.get("oneOf").is_some()
                || definition.get("anyOf").is_some())
        {
            assert!(
                definition.get("x-p4kx-vocabularyClass").is_none(),
                "the actual compound source envelope {name} is not an open or control scalar vocabulary"
            );
            compounds += 1;
        }
    }
    assert!(
        compounds >= 3,
        "the schema contains the actual recursive source envelopes"
    );
    Ok(())
}

fn open_labels_closed_controls(validator: &jsonschema::Validator, catalog: &Value) -> TestResult {
    let mut unknown = catalog.clone();
    *unknown
        .pointer_mut(&format!(
            "{}/displayTransformation/value/nodes/0/payload/value/type",
            support::FIRST
        ))
        .ok_or("actual closed operation control")? = json!("futureSuccess");
    assert!(
        !validator.is_valid(&unknown),
        "an unknown operation control is never a successful payload shape"
    );
    let mut open = catalog.clone();
    *open
        .pointer_mut(&format!(
            "{}/displayTransformation/value/nodes/0/sourceType/value",
            support::FIRST
        ))
        .ok_or("actual source type cell")? = json!("SyntheticFutureType");
    *open.pointer_mut(&format!("{}/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes/value/0/value",
        support::FIRST)).ok_or("actual original Enum cell")? = json!("SyntheticFutureLabel");
    assert!(
        validator.is_valid(&open),
        "actual source-type and Enum strings remain open game data"
    );
    Ok(())
}

fn structural_controls(validator: &jsonschema::Validator, catalog: &Value) -> TestResult {
    required_groups(validator, catalog)?;
    control_indices(validator, catalog)?;
    let graph = support::at(support::row(catalog, 0)?, "/displayTransformation/value")?;
    assert!(
        graph.get("state").is_none(),
        "the exporter arena has no fabricated source carrier"
    );
    assert!(
        support::at(graph, "/root")?.get("state").is_none(),
        "a local position is a structural control"
    );
    assert!(
        support::at(graph, "/nodes/0")?.get("state").is_none(),
        "an occurrence group is not another source slot"
    );
    Ok(())
}

fn required_groups(validator: &jsonschema::Validator, catalog: &Value) -> TestResult {
    for (owner, field) in [
        ("/displayTransformation/value", "root"),
        ("/displayTransformation/value", "nodes"),
        ("/displayTransformation/value/root", "nodeIndex"),
        (
            "/displayTransformation/value/nodes/0/payload/value/transformations/value/0/value",
            "nodeIndex",
        ),
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut(&format!("{}{owner}", support::FIRST))
            .and_then(Value::as_object_mut)
            .ok_or("actual structural group")?
            .remove(field)
            .ok_or("required structural key")?;
        assert!(
            !validator.is_valid(&missing),
            "the local structure requires {field} at {owner}"
        );
    }
    Ok(())
}

fn control_indices(validator: &jsonschema::Validator, catalog: &Value) -> TestResult {
    for invalid in [json!(-1), json!(4_294_967_296_u64), json!(0.5), json!("0")] {
        let mut wrong = catalog.clone();
        *wrong
            .pointer_mut(&format!(
                "{}/displayTransformation/value/root/nodeIndex",
                support::FIRST
            ))
            .ok_or("actual local root control")? = invalid;
        assert!(
            !validator.is_valid(&wrong),
            "a local control has the exact u32 integer schema"
        );
    }
    Ok(())
}

fn required_cell(validator: &jsonschema::Validator, catalog: &Value, relative: &str) -> TestResult {
    for key in [
        "state",
        "value",
        "sourceUsesRefs",
        "sourceSetComplete",
        "containsEncryptedSource",
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut(&format!("{}{relative}", support::FIRST))
            .and_then(Value::as_object_mut)
            .ok_or("actual nested source cell")?
            .remove(key)
            .ok_or("required immediate source key")?;
        assert!(
            !validator.is_valid(&missing),
            "every recursive data cell requires {key} at {relative}"
        );
    }
    Ok(())
}
