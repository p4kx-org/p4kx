// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{SourceUses, SourceValue, SourcedValue};
use schemars::{JsonSchema, SchemaGenerator};
use serde::Serialize;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn adding_unrelated_carriers_preserves_existing_references_and_definitions() -> TestResult {
    let mut initial = SchemaGenerator::default();
    let expected = initial.subschema_for::<SourcedValue<u32>>();
    let name = SourcedValue::<u32>::schema_name();
    let definition = initial
        .definitions()
        .get(name.as_ref())
        .ok_or("initial carrier definition")?;
    let mut expanded = SchemaGenerator::default();
    expanded.subschema_for::<SourcedValue<String>>();
    expanded.subschema_for::<SourcedValue<bool>>();
    expanded.subschema_for::<SourcedValue<Option<u32>>>();
    let actual = expanded.subschema_for::<SourcedValue<u32>>();
    assert_eq!(
        actual, expected,
        "adding payload types must not renumber an existing field reference"
    );
    assert_eq!(
        expanded.definitions().get(name.as_ref()),
        Some(definition),
        "adding payload types must preserve the existing carrier contract"
    );
    Ok(())
}

#[test]
fn borrowed_and_owned_text_reuse_one_schema_identity_and_reference() {
    assert_eq!(
        SourcedValue::<String>::schema_id(),
        SourcedValue::<&str>::schema_id(),
        "borrowed text and owned text have the same wire contract"
    );
    let mut generator = SchemaGenerator::default();
    let owned = generator.subschema_for::<SourcedValue<String>>();
    let borrowed = generator.subschema_for::<SourcedValue<&str>>();
    assert_eq!(
        borrowed, owned,
        "ownership must not duplicate the text carrier definition"
    );
    assert_eq!(
        generator
            .definitions()
            .keys()
            .filter(|name| name.starts_with("SourcedValue"))
            .count(),
        1,
        "both ownership forms must reuse one carrier definition"
    );
}

mod number {
    use schemars::JsonSchema;
    use serde::Serialize;

    #[derive(JsonSchema, Serialize)]
    pub(super) struct Collision {
        pub(super) number: u32,
    }
}

mod flag {
    use schemars::JsonSchema;
    use serde::Serialize;

    #[derive(JsonSchema, Serialize)]
    pub(super) struct Collision {
        pub(super) flag: bool,
    }
}

#[derive(JsonSchema, Serialize)]
struct CollidingCarriers {
    number: SourcedValue<number::Collision>,
    flag: SourcedValue<flag::Collision>,
}

#[test]
fn equal_payload_names_keep_distinct_schema_ids_and_payload_validation() -> TestResult {
    let mut generator = SchemaGenerator::default();
    let number = generator.subschema_for::<SourcedValue<number::Collision>>();
    let flag = generator.subschema_for::<SourcedValue<flag::Collision>>();
    assert_ne!(
        number, flag,
        "matching display names must not merge different schema identities"
    );
    let schema = schemars::schema_for!(CollidingCarriers);
    let validator = jsonschema::validator_for(&schema.to_value())?;
    let original = serde_json::to_value(CollidingCarriers {
        number: literal(number::Collision { number: 0 }),
        flag: literal(flag::Collision { flag: false }),
    })?;
    assert!(
        validator.is_valid(&original),
        "both different payload contracts must remain reachable"
    );
    for pointer in ["/number/value/number", "/flag/value/flag"] {
        let mut wrong = original.clone();
        *wrong.pointer_mut(pointer).ok_or("actual typed payload")? = json!("wrong type");
        assert!(
            !validator.is_valid(&wrong),
            "the concrete contract remains enforced at {pointer}"
        );
    }
    Ok(())
}

#[derive(JsonSchema, Serialize)]
struct RecursiveNode {
    next: Option<Box<SourcedValue<Self>>>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "A/B~C")]
struct EscapedName {
    number: u32,
}

#[derive(JsonSchema, Serialize)]
struct LinkedCarriers {
    node: SourcedValue<RecursiveNode>,
    escaped: SourcedValue<EscapedName>,
}

#[test]
fn recursive_and_escaped_references_keep_the_complete_carrier_contract() -> TestResult {
    let schema = schemars::schema_for!(LinkedCarriers);
    let validator = jsonschema::validator_for(&schema.to_value())?;
    let leaf = literal(RecursiveNode { next: None });
    let original = serde_json::to_value(LinkedCarriers {
        node: literal(RecursiveNode {
            next: Some(Box::new(leaf)),
        }),
        escaped: literal(EscapedName { number: 0 }),
    })?;
    assert!(
        validator.is_valid(&original),
        "recursive and escaped references must resolve correctly"
    );
    let mut wrong = original.clone();
    *wrong
        .pointer_mut("/escaped/value/number")
        .ok_or("escaped-name payload")? = json!(false);
    assert!(
        !validator.is_valid(&wrong),
        "escaped references must enforce their concrete payload type"
    );
    let mut wrong = original;
    wrong
        .pointer_mut("/node/value/next")
        .and_then(Value::as_object_mut)
        .ok_or("nested recursive carrier")?
        .remove("sourceSetComplete");
    assert!(
        !validator.is_valid(&wrong),
        "recursive references must retain required origin metadata"
    );
    Ok(())
}

fn literal<T>(value: T) -> SourcedValue<T> {
    SourceUses::new().value(SourceValue::Present(value), &[], true)
}
