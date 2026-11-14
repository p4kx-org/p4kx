// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use jsonschema::Validator;
use serde_json::{Value, json};

use crate::catalog_schema_support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const ROW: &str = "/definitions/gameplayProperties/rows/0";
const DISPLAY: &str = "/displayTransformation/value/nodes";

#[test]
fn the_full_gameplay_schema_fixture_keeps_its_identity_texts_and_source_states() -> TestResult {
    let catalog = catalog_schema_support::minimal_catalog();
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the complete positive catalog fixture is valid"
    );
    assert!(
        catalog.get("gameplayProperties").is_none(),
        "there is one canonical property table"
    );
    for (field, expected) in [
        ("guid", "e1000000-0000-4000-8000-000000000001"),
        ("className", "GPP_Armor"),
        (
            "recordPath",
            "libs/foundry/records/crafting/craftedproperties/gpp_armor",
        ),
        ("propertyName", "name:19:@StatName_GPP_Armor"),
    ] {
        assert_eq!(
            at(&catalog, &format!("{ROW}/{field}/value"))?,
            &json!(expected),
            "the existing synthetic {field} remains concrete"
        );
    }
    assert_text(&catalog)?;
    assert_eq!(
        at(&catalog, "/exportHealth/kinds")?
            .as_array()
            .ok_or("health kinds")?
            .iter()
            .find(|kind| kind.get("kind").and_then(Value::as_str) == Some("gameplayProperties"))
            .and_then(|kind| kind.get("writtenRows")),
        Some(&json!(1)),
        "fixture accounting counts the canonical definition"
    );
    Ok(())
}

fn assert_text(catalog: &Value) -> TestResult {
    let names = at(catalog, "/texts/publicNames")?
        .as_array()
        .ok_or("public names")?;
    let mut found = None;
    for name in names {
        if name.pointer("/id/value").and_then(Value::as_str) == Some("name:19:@StatName_GPP_Armor")
            && found.replace(name).is_some()
        {
            return Err("one constructed property text row".into());
        }
    }
    let name = found.ok_or("the constructed Armour text row")?;
    assert_eq!(
        at(name, "/key/value")?,
        &json!("@StatName_GPP_Armor"),
        "the old concrete key is retained"
    );
    for form in ["sourceText", "displayText"] {
        assert_eq!(
            at(name, &format!("/translations/value/en/{form}/value"))?,
            &json!("Armour"),
            "the constructed English {form}"
        );
    }
    assert!(
        at(name, "/translations/value")?.get("de").is_none(),
        "the constructed sample does not invent a German lookup state from its old null result"
    );
    Ok(())
}

#[test]
fn gameplay_schema_fixture_requires_all_headers_fields_and_direct_nested_carriers() -> TestResult {
    let catalog = catalog_schema_support::minimal_catalog();
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "negative mutations start from the complete positive fixture"
    );
    for field in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "fileName",
        "metadataTag",
        "propertyName",
        "displayTransformation",
        "unitFormat",
        "nameOverrides",
    ] {
        remove(&validator, &catalog, ROW, field)?;
    }
    for path in [
        format!("{ROW}/propertyName"),
        format!("{ROW}{DISPLAY}/0/sourceType"),
        format!("{ROW}{DISPLAY}/0/payload"),
        format!("{ROW}{DISPLAY}/0/payload/value/transformations/value/0"),
        format!("{ROW}{DISPLAY}/1/payload/value/scale"),
        format!(
            "{ROW}/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes/value/0"
        ),
    ] {
        for field in [
            "state",
            "value",
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete",
        ] {
            remove(&validator, &catalog, &path, field)?;
        }
    }
    Ok(())
}

#[test]
fn gameplay_schema_fixture_keeps_open_labels_and_closed_operation_and_local_link_controls()
-> TestResult {
    let catalog = catalog_schema_support::minimal_catalog();
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(validator.is_valid(&catalog), "the baseline is valid");
    let mut open = catalog.clone();
    *at_mut(&mut open, &format!("{ROW}{DISPLAY}/0/sourceType/value"))? =
        json!("SyntheticFutureClass");
    *at_mut(
        &mut open,
        &format!(
            "{ROW}/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes/value/0/value"
        ),
    )? = json!("SyntheticFutureLabel");
    assert!(
        validator.is_valid(&open),
        "original classes and Enum labels remain open data"
    );
    for wrong in [json!(-1), json!(u64::from(u32::MAX) + 1), json!("1")] {
        let mut changed = catalog.clone();
        *at_mut(
            &mut changed,
            &format!("{ROW}/displayTransformation/value/root/nodeIndex"),
        )? = wrong;
        assert!(
            !validator.is_valid(&changed),
            "local structural indices use exact u32 storage"
        );
    }
    let mut changed = catalog;
    *at_mut(
        &mut changed,
        &format!("{ROW}{DISPLAY}/0/payload/value/type"),
    )? = json!("futureOperation");
    assert!(
        !validator.is_valid(&changed),
        "exporter operation tags remain closed controls"
    );
    Ok(())
}

fn remove(validator: &Validator, catalog: &Value, pointer: &str, field: &str) -> TestResult {
    let mut changed = catalog.clone();
    changed
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or("actual constructed object")?
        .remove(field)
        .ok_or("existing required field")?;
    assert!(
        !validator.is_valid(&changed),
        "required {pointer}/{field} cannot be omitted"
    );
    Ok(())
}

fn at<'a>(value: &'a Value, pointer: &str) -> TestResult<&'a Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("constructed fixture value {pointer}").into())
}

fn at_mut<'a>(value: &'a mut Value, pointer: &str) -> TestResult<&'a mut Value> {
    value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("constructed mutable fixture value {pointer}").into())
}
