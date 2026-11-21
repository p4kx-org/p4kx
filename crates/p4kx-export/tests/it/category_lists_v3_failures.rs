// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::{DataForge, DataForgeError, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_lists_v3_retains_headers_and_exact_diagnosis_for_unknown_authored_body() -> TestResult {
    let database = unknown_source();
    assert_unknown_source(&database)?;
    fixture::refused(&database, None)?;
    let (built, catalog) = fixture::diagnostic(&database)?;
    fixture::count(&built, 1, 0)?;
    let row = fixture::row(&catalog, 0)?;
    assert_eq!(
        fixture::at(row, "/recordName/value")?,
        &json!("BlueprintCategoryDatabaseRecord.Primary"),
        "body $name cannot overwrite the original header"
    );
    assert!(
        !row.as_object().ok_or("list row")?.contains_key("$name"),
        "unhandled fields are not hidden in an untyped dump"
    );
    assert_health(&catalog, 1, 1, 0)?;
    assert_diagnosis(&catalog, "unsupportedValue", "$name")?;
    assert!(
        built.summary().failures().is_empty(),
        "field coverage failure is not a whole-source omission"
    );
    Ok(())
}

fn unknown_source() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(fixture::CATEGORY_TYPE))
        .structure(fixture::list_schema().field("$name", TestType::String))
        .record(
            fixture::listing(
                fixture::LIST,
                "Primary",
                fixture::LIST_FILE,
                fixture::references(&[]),
            )
            .value("$name", TestValue::text("BodyOnly")),
        )
}

fn assert_unknown_source(database: &DataForgeBuilder) -> TestResult {
    source::header(
        database,
        0,
        fixture::LIST,
        "BlueprintCategoryDatabaseRecord.Primary",
        fixture::LIST_FILE,
        "PrimaryDomain",
    )?;
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original body owner")?.tree()?;
    assert_eq!(
        tree.root().field("$name"),
        Some(&Raw::String("BodyOnly")),
        "the unknown body fact is genuinely readable"
    );
    Ok(())
}

#[test]
fn category_lists_v3_reports_strict_read_failure_and_filters_before_reading_a_broken_list()
-> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(fixture::CATEGORY_TYPE))
        .structure(fixture::list_schema().declared_size(9))
        .record(fixture::listing(
            fixture::LIST,
            "Primary",
            fixture::LIST_FILE,
            fixture::references(&[]),
        ));
    assert_broken_source(&database)?;
    fixture::refused(&database, None)?;
    let (built, catalog) = fixture::diagnostic(&database)?;
    fixture::count(&built, 0, 0)?;
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "a strict read failure cannot create a list row"
    );
    assert_health(&catalog, 0, 0, 1)?;
    assert_failed_physical(&catalog)?;
    assert_filtered_broken(&database)?;
    Ok(())
}

fn assert_broken_source(database: &DataForgeBuilder) -> TestResult {
    source::header(
        database,
        0,
        fixture::LIST,
        "BlueprintCategoryDatabaseRecord.Primary",
        fixture::LIST_FILE,
        "PrimaryDomain",
    )?;
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    assert!(
        matches!(
            forge.record(0).ok_or("original broken list")?.tree(),
            Err(DataForgeError::StructLayout {
                declared: 9,
                fields: 8,
                ..
            })
        ),
        "the original format reader proves the intended strict failure"
    );
    Ok(())
}

fn assert_filtered_broken(database: &DataForgeBuilder) -> TestResult {
    let filter = Filter::new("*/excluded/*");
    let (built, catalog) = fixture::build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    fixture::count(&built, 0, 0)?;
    assert!(
        built.summary().failures().is_empty(),
        "an excluded database is not read"
    );
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "the excluded database remains unmaterialized"
    );
    assert_eq!(
        fixture::at(
            &catalog,
            "/declarations/blueprintCategoryLists/availability"
        )?,
        &json!("available"),
        "read failure and Filter do not erase source-family declaration"
    );
    Ok(())
}

fn assert_health(catalog: &Value, read: u32, projected: u32, failed: u32) -> TestResult {
    let health = fixture::health(catalog)?;
    for (path, expected) in [
        ("/sourceRecords/readSuccessful", read),
        ("/sourceRecords/projected", projected),
        ("/sourceRecords/omittedAfterReadFailure", failed),
        ("/sourceRecords/omittedAfterProjectionFailure", 0),
    ] {
        assert_eq!(
            fixture::at(health, path)?,
            &json!(expected),
            "list read/projection/omission accounting has one physical owner"
        );
    }
    Ok(())
}

fn assert_diagnosis(catalog: &Value, code: &str, field: &str) -> TestResult {
    let diagnostics = fixture::at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("list diagnostics")?;
    let mut found = false;
    for diagnostic in diagnostics {
        if diagnostic.get("kind").and_then(Value::as_str) != Some(fixture::KIND)
            || diagnostic.get("code").and_then(Value::as_str) != Some(code)
        {
            continue;
        }
        let examples = diagnostic["examples"].as_array().ok_or("list examples")?;
        for example in examples {
            found |= example
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(0)
                && crate::health_origin_assertions::example_text(catalog, example, "field")?
                    == Some(&json!(field))
                && crate::health_origin_assertions::example_text(catalog, example, "sourceType")?
                    == Some(&json!(fixture::TYPE));
        }
    }
    assert!(
        found,
        "the field failure retains exact original owner, type and field"
    );
    Ok(())
}

fn assert_failed_physical(catalog: &Value) -> TestResult {
    let guid = common::guid(fixture::LIST);
    let failures = fixture::at(catalog, "/exportHealth/failedRecords")?
        .as_array()
        .ok_or("failed physical Records")?;
    assert!(
        failures.iter().any(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(0)
                && row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str())
                && row.get("kind").and_then(Value::as_str) == Some(fixture::KIND)
                && row.get("role").and_then(Value::as_str) == Some("selected")
                && row.get("code").and_then(Value::as_str) == Some("sourceReadFailure")
        }),
        "strict failure retains physical identity and its own selected Kind"
    );
    Ok(())
}
