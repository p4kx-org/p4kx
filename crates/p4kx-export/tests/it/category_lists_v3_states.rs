// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::category_lists_v3_fixture as fixture;
use crate::commodity_v3_fixture::TestResult;

#[test]
fn category_lists_v3_keeps_each_real_array_source_state_without_false_empty_success() -> TestResult
{
    let mut unreadable = 1_u32.to_le_bytes().to_vec();
    unreadable.extend_from_slice(&u32::MAX.to_le_bytes());
    for (state, declaration, value) in [
        ("absent", None, None),
        (
            "null",
            Some((TestType::StrongPointer("Empty".to_owned()), false)),
            Some(TestValue::Null),
        ),
        (
            "present",
            Some((
                TestType::ReferenceTo(fixture::CATEGORY_TYPE.to_owned()),
                true,
            )),
            Some(TestValue::Array(vec![])),
        ),
        (
            "invalid",
            Some((TestType::Bool, false)),
            Some(TestValue::Bool(false)),
        ),
        (
            "unreadable",
            Some((
                TestType::ReferenceTo(fixture::CATEGORY_TYPE.to_owned()),
                true,
            )),
            Some(TestValue::Raw(unreadable)),
        ),
    ] {
        check_state(state, declaration, value)?;
    }
    Ok(())
}

fn check_state(
    state: &str,
    declaration: Option<(TestType, bool)>,
    value: Option<TestValue>,
) -> TestResult {
    let database = database(declaration, value);
    assert_original_state(&database, state)?;
    let (built, catalog) = fixture::diagnostic(&database)?;
    fixture::count(&built, 1, 0)?;
    assert_exported_state(&catalog, state)?;
    if matches!(state, "invalid" | "unreadable") {
        fixture::refused(&database, None)?;
        assert_eq!(
            fixture::at(&catalog, "/exportHealth/status")?,
            &json!("incomplete"),
            "bad field coverage cannot publish as complete"
        );
    }
    Ok(())
}

fn database(declaration: Option<(TestType, bool)>, value: Option<TestValue>) -> DataForgeBuilder {
    let mut listing = TestStruct::new(fixture::TYPE);
    if let Some((kind, array)) = declaration {
        listing = if array {
            listing.array("categories", kind)
        } else {
            listing.field("categories", kind)
        };
    }
    let mut record = TestRecord::new(
        fixture::LIST,
        fixture::TYPE,
        "BlueprintCategoryDatabaseRecord.State",
        fixture::LIST_FILE,
    );
    if let Some(value) = value {
        record = record.value("categories", value);
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty"))
        .structure(TestStruct::new(fixture::CATEGORY_TYPE))
        .structure(listing)
        .record(record)
}

fn assert_exported_state(catalog: &Value, state: &str) -> TestResult {
    let row = fixture::row(catalog, 0)?;
    assert_eq!(
        fixture::at(row, "/categories/state")?,
        &json!(state),
        "the real source state survives without a default empty list"
    );
    let expected = if state == "present" {
        json!([])
    } else {
        Value::Null
    };
    assert_eq!(
        fixture::at(row, "/categories/value")?,
        &expected,
        "only an original present empty array becomes []"
    );
    assert_eq!(
        fixture::at(fixture::health(catalog)?, "/sourceRecords/projected")?,
        &json!(1),
        "a field failure does not discard the readable physical header"
    );
    Ok(())
}

fn assert_original_state(database: &DataForgeBuilder, state: &str) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("original state owner")?;
    assert_eq!(
        record.type_name()?,
        fixture::TYPE,
        "the actual physical owner is a category list"
    );
    let tree = record.tree()?;
    let matches = match (state, tree.root().field("categories")) {
        ("absent", None) | ("null", Some(Raw::Null)) | ("invalid", Some(Raw::Bool(false))) => true,
        ("present", Some(Raw::Array(values))) => values.is_empty(),
        ("unreadable", Some(Raw::Error(ValueError::ArrayOutOfRange { first, count, .. }))) => {
            *first == u32::MAX && *count == 1
        }
        _ => false,
    };
    assert!(
        matches,
        "the intended source state is genuinely encoded before export"
    );
    Ok(())
}
