// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::{DataForge, DataForgeError, Value as SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_v3_retains_readable_headers_but_refuses_an_unhandled_authored_body_field() -> TestResult
{
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(fixture::SOURCE_TYPE).field("$name", TestType::String))
        .record(
            fixture::category(fixture::CATEGORY, fixture::RAW_NAME, fixture::FILE_NAME)
                .value("$name", TestValue::text("BodyOnly")),
        );
    fixture::original(
        &source,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original source category")?.tree()?;
    assert_eq!(
        tree.root().field("$name"),
        Some(&SourceValue::String("BodyOnly")),
        "the unsupported source field is really present and readable"
    );
    fixture::refused(&source)?;
    let (built, catalog) = fixture::diagnostic(&source)?;
    fixture::counts(&built, 1, 0)?;
    assert!(
        built.summary().failures().is_empty(),
        "a field diagnosis is not a whole-source read failure"
    );
    let row = fixture::row(&catalog, 0)?;
    assert_eq!(
        fixture::at(row, "/recordName/value")?,
        &json!(fixture::RAW_NAME),
        "body $name cannot supply or overwrite the original header name"
    );
    assert!(
        !row.as_object().ok_or("category row")?.contains_key("$name"),
        "no generic raw body dump is invented"
    );
    assert_unhandled_field_health(&catalog)?;
    Ok(())
}

#[test]
fn category_v3_keeps_strict_source_failures_and_does_not_read_filtered_broken_categories()
-> TestResult {
    let source = broken_source()?;
    fixture::refused(&source)?;
    let (built, catalog) = fixture::diagnostic(&source)?;
    fixture::counts(&built, 0, 0)?;
    let failure = built
        .summary()
        .failures()
        .first()
        .ok_or("original category read failure")?;
    assert_eq!(
        failure.guid.to_string(),
        common::guid(fixture::CATEGORY),
        "the failure retains the original GUID"
    );
    assert_eq!(
        failure.record_path,
        fixture::PATH,
        "the failure retains the normalized source path"
    );
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "strict failure cannot fabricate a category row"
    );
    let health = fixture::health(&catalog)?;
    for (path, expected) in [
        ("/sourceRecords/readSuccessful", 0),
        ("/sourceRecords/projected", 0),
        ("/sourceRecords/omittedAfterReadFailure", 1),
        ("/sourceRecords/omittedAfterProjectionFailure", 0),
    ] {
        assert_eq!(
            fixture::at(health, path)?,
            &json!(expected),
            "the failed read is accounted exactly once"
        );
    }
    assert_physical_failure(&catalog)?;
    assert_filtered_category_is_unread(&source)?;
    Ok(())
}

fn assert_physical_failure(catalog: &Value) -> TestResult {
    let expected_guid = common::guid(fixture::CATEGORY);
    let failed = fixture::at(catalog, "/exportHealth/failedRecords")?
        .as_array()
        .ok_or("failed physical Records")?;
    assert!(
        failed.iter().any(|entry| {
            entry.get("recordIndex").and_then(Value::as_u64) == Some(0)
                && entry.get("guid").and_then(Value::as_str) == Some(expected_guid.as_str())
                && entry.get("kind").and_then(Value::as_str) == Some("blueprintCategories")
                && entry.get("role").and_then(Value::as_str) == Some("selected")
                && entry.get("code").and_then(Value::as_str) == Some("sourceReadFailure")
        }),
        "the failure keeps actual physical identity, role, kind and source-read category"
    );
    Ok(())
}

fn assert_unhandled_field_health(catalog: &Value) -> TestResult {
    let health = fixture::health(catalog)?;
    for (path, expected) in [
        ("/sourceRecords/readSuccessful", 1),
        ("/sourceRecords/projected", 1),
        ("/sourceRecords/omittedAfterReadFailure", 0),
        ("/sourceRecords/omittedAfterProjectionFailure", 0),
    ] {
        assert_eq!(
            fixture::at(health, path)?,
            &json!(expected),
            "the readable physical row and its field failure are accounted separately"
        );
    }
    let diagnostics = fixture::at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("field diagnostics")?;
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.get("code").and_then(Value::as_str) == Some("unsupportedValue")
                && diagnostic.get("kind").and_then(Value::as_str) == Some("blueprintCategories")
                && diagnostic
                    .get("examples")
                    .and_then(Value::as_array)
                    .is_some_and(|examples| {
                        examples.iter().any(|example| {
                            example.get("recordIndex").and_then(Value::as_u64) == Some(0)
                                && example.get("field").and_then(Value::as_str) == Some("$name")
                                && example.get("sourceType").and_then(Value::as_str)
                                    == Some(fixture::SOURCE_TYPE)
                        })
                    })
        }),
        "unsupported coverage keeps the actual physical owner, body field and source type"
    );
    Ok(())
}

fn assert_filtered_category_is_unread(source: &DataForgeBuilder) -> TestResult {
    let filter = Filter::new("*/outside/*");
    let (filtered, catalog) = fixture::build(
        source,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    fixture::counts(&filtered, 0, 0)?;
    assert!(
        filtered.summary().failures().is_empty(),
        "a filtered category is not read and cannot add a source failure"
    );
    assert!(
        fixture::rows(&catalog)?.is_empty(),
        "the excluded category stays unmaterialized"
    );
    assert_eq!(
        fixture::at(&catalog, "/definitions/blueprintCategories/availability")?,
        &json!("available"),
        "failed or filtered rows do not erase the declared family"
    );
    Ok(())
}

fn broken_source() -> TestResult<DataForgeBuilder> {
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(fixture::SOURCE_TYPE)
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(fixture::category(
            fixture::CATEGORY,
            fixture::RAW_NAME,
            fixture::FILE_NAME,
        ));
    fixture::original(
        &source,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    assert!(
        matches!(
            forge.record(0).ok_or("original broken category")?.tree(),
            Err(DataForgeError::StructLayout {
                declared: 3,
                fields: 1,
                ..
            })
        ),
        "the actual original reader rejects the malformed declared layout"
    );
    Ok(source)
}
