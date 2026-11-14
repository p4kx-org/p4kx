// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{ContractScopeGlobalParams, ITEM_TYPE, SOURCE_TYPE};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::record_observation::RecordObservation;

const FUTURE: &str = "FixtureDerivedContractScope";
const UNRELATED: &str = "FixtureUnrelatedContractScope";
const DISPLAY_NAME: &str = "@FiXtureScope";
const ICON: &str = "Fixture/Scope/Icon";
const SLOTS: [&str; 7] = [
    "location",
    "cluster",
    "sector",
    "planetaryBody",
    "planetarySystem",
    "solarSystem",
    "universe",
];

#[test]
fn contract_scope_retains_all_slots_exact_public_keys_and_derived_fields() -> TestResult {
    project(&fixture(ITEM_TYPE).build(), |row, texts, notes| {
        assert_eq!(notes, 0, "a descendant of the declared item is readable");
        let name = check_present(texts, "/publicNames/0/key", &json!(DISPLAY_NAME))?;
        let name_id = at(texts, "/publicNames/0/id/value")?;
        assert!(
            at(texts, "/gameTexts")?
                .as_array()
                .is_some_and(Vec::is_empty),
            "the short display name uses only the public-name registry"
        );
        let mut previous_receipts = None;
        for slot in SLOTS {
            let path = format!("/{slot}");
            let carrier = at(row, &path)?;
            assert_eq!(at(carrier, "/state")?, "present", "{slot} is represented");
            let receipts = at(carrier, "/sourceUsesRefs")?;
            assert_ne!(
                previous_receipts,
                Some(receipts),
                "distinct authored pointer occurrence"
            );
            previous_receipts = Some(receipts);
            let item = at(carrier, "/value")?;
            check_present(item, "/displayName", name_id)?;
            check_present(item, "/icon", &json!(ICON))?;
            let expected_type = if slot == "sector" { FUTURE } else { ITEM_TYPE };
            check_present(item, "/sourceType", &json!(expected_type))?;
        }
        check_raw_boolean(at(row, "/sector/value/additionalFields")?, "fixtureExtra")?;
        check_raw_boolean(at(row, "/additionalFields")?, "fixtureRootExtra")?;
        assert_eq!(
            at(name, "/sourceUsesRefs")?.as_array().map(Vec::len),
            Some(SLOTS.len()),
            "the deduplicated exact public key has one payload per original display name"
        );
        Ok(())
    })
}

#[test]
fn contract_scope_distinguishes_wrong_family_null_and_missing_declarations() -> TestResult {
    project(&fixture(UNRELATED).build(), |row, texts, notes| {
        assert_eq!(notes, 1, "only the rejected declaration needs a note");
        assert_eq!(
            at(row, "/location/state")?,
            "unsupported",
            "wrong nominal family"
        );
        assert_eq!(at(row, "/cluster/state")?, "null", "declared null pointer");
        assert_eq!(at(row, "/universe/state")?, "absent", "omitted declaration");
        assert_eq!(
            at(row, "/additionalFields/roots/0/name/value")?,
            "location",
            "wrong-family occurrence remains in the original source arena"
        );
        assert_eq!(
            at(
                row,
                "/additionalFields/values/0/value/value/sourceType/value"
            )?,
            UNRELATED,
            "the rejected concrete source structure survives"
        );
        assert!(
            at(texts, "/publicNames")?
                .as_array()
                .is_some_and(Vec::is_empty),
            "rejected pointers produce no invented display-name keys"
        );
        Ok(())
    })
}

fn project(bytes: &[u8], verify: impl FnOnce(&Value, &Value, usize) -> TestResult) -> TestResult {
    test_support::with_projection(bytes, |forge, projection| {
        let record = forge.record(0).ok_or("original scope Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let (prepared, notes) = PreparedDefinition::read(
            &tree,
            projection,
            &admission,
            None,
            ContractScopeGlobalParams::read,
        )?;
        let table = PreparedDefinition::table(forge, SOURCE_TYPE, vec![prepared]);
        let row = serde_json::to_value(
            table
                .rows
                .first()
                .ok_or("projected physical scope Record")?,
        )?;
        let texts = serde_json::to_value(projection.finish()?)?;
        verify(&row, &texts, notes.len())
    })
}

fn fixture(location_type: &str) -> DataForgeBuilder {
    let root = TestStruct::new(SOURCE_TYPE)
        .field("location", pointer(location_type))
        .field("cluster", pointer(ITEM_TYPE));
    let mut record = TestRecord::new(
        1,
        SOURCE_TYPE,
        "ContractScopeGlobalParams.Fixture",
        "fixture/scope.xml",
    );
    let root = if location_type == ITEM_TYPE {
        for slot in SLOTS {
            let concrete = if slot == "sector" { FUTURE } else { ITEM_TYPE };
            record = record.value(slot, item(concrete));
        }
        record = record.value("fixtureRootExtra", TestValue::Bool(true));
        SLOTS
            .into_iter()
            .skip(2)
            .fold(root, |root, slot| root.field(slot, pointer(ITEM_TYPE)))
            .field("fixtureRootExtra", TestType::Bool)
    } else {
        record = record.value("location", TestValue::Struct(TestInstance::new(UNRELATED)));
        root
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(root)
        .structure(
            TestStruct::new(ITEM_TYPE)
                .field("displayName", TestType::Locale)
                .field("icon", TestType::String),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(ITEM_TYPE)
                .field("fixtureExtra", TestType::Bool),
        )
        .structure(TestStruct::new(UNRELATED))
        .record(record)
}

fn item(concrete: &str) -> TestValue {
    let item = TestInstance::new(concrete)
        .value("displayName", TestValue::text(DISPLAY_NAME))
        .value("icon", TestValue::text(ICON));
    let item = if concrete == FUTURE {
        item.value("fixtureExtra", TestValue::Bool(true))
    } else {
        item
    };
    TestValue::Struct(item)
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn check_raw_boolean(arena: &Value, field: &str) -> TestResult {
    assert_eq!(
        at(arena, "/roots/0/name/value")?,
        field,
        "exact retained field"
    );
    assert_eq!(
        at(arena, "/roots/0/value/valueIndex")?,
        "0",
        "local arena reference"
    );
    assert_eq!(
        at(arena, "/values/0/value/value")?,
        &json!({"type": "boolean", "value": true}),
        "original boolean payload is exact"
    );
    Ok(())
}

fn check_present<'value>(
    row: &'value Value,
    path: &str,
    expected: &Value,
) -> TestResult<&'value Value> {
    let carrier = at(row, path)?;
    assert_eq!(at(carrier, "/state")?, "present", "{path} is present");
    assert_eq!(
        at(carrier, "/value")?,
        expected,
        "{path} retains original meaning"
    );
    assert_eq!(
        at(carrier, "/containsEncryptedSource")?,
        false,
        "{path} is backed by the plain fixture"
    );
    assert_eq!(
        at(carrier, "/sourceSetComplete")?,
        true,
        "{path} source set is complete"
    );
    assert!(
        at(carrier, "/sourceUsesRefs")?
            .as_array()
            .is_some_and(|refs| !refs.is_empty()),
        "{path} retains directly paired source receipts"
    );
    Ok(carrier)
}

fn at<'value>(row: &'value Value, path: &str) -> TestResult<&'value Value> {
    row.pointer(path)
        .ok_or_else(|| format!("missing fixture field {path}").into())
}
