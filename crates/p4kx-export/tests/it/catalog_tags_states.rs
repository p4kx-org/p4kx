// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CancelToken, CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin};
use crate::health_origin_assertions;

#[test]
fn absent_null_empty_invalid_and_unreadable_entity_arrays_remain_distinct() -> TestResult {
    let mut invalid_array = 1_u32.to_le_bytes().to_vec();
    invalid_array.extend_from_slice(&u32::MAX.to_le_bytes());
    for (state, ty, array, value) in [
        ("absent", None, false, None),
        (
            "null",
            Some(TestType::StrongPointer("FixtureEmpty".to_owned())),
            false,
            Some(TestValue::Null),
        ),
        (
            "present",
            Some(TestType::ReferenceTo("Tag".to_owned())),
            true,
            Some(fixture::references(&[])),
        ),
        (
            "invalid",
            Some(TestType::Bool),
            false,
            Some(TestValue::Bool(false)),
        ),
        (
            "unreadable",
            Some(TestType::ReferenceTo("Tag".to_owned())),
            true,
            Some(TestValue::Raw(invalid_array)),
        ),
    ] {
        assert_entity_array_state(state, ty, array, value)?;
    }
    Ok(())
}

fn assert_entity_array_state(
    state: &str,
    ty: Option<TestType>,
    array: bool,
    value: Option<TestValue>,
) -> TestResult {
    let database = fixture::entity_schema(fixture::schema(), ty, array).record(fixture::entity(
        fixture::ITEM,
        "State",
        "scitem/ships/powerplant/state",
        value,
    ));
    let (_, catalog) = fixture::diagnostic(&database)?;
    let row = fixture::rows(&catalog, "declarations/entityTags")?
        .first()
        .ok_or("entity declaration")?;
    assert_eq!(
        at(row, "/tags/state")?,
        &json!(state),
        "each Entity array source state remains distinct"
    );
    if state == "present" {
        assert_eq!(
            at(row, "/tags/value")?,
            &json!([]),
            "a present empty Entity array stays present and empty"
        );
    } else {
        assert!(
            at(row, "/tags/value")?.is_null(),
            "nonpresent array state retains a required null payload"
        );
    }
    if matches!(state, "invalid" | "unreadable") {
        let result = fixture::build(
            &database,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            None,
        );
        assert!(
            result.is_err(),
            "incomplete authored source cannot publish normally"
        );
    }
    Ok(())
}

#[test]
fn canonical_tag_read_failure_is_health_evidence_without_a_fabricated_definition() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .declared_size(6),
        )
        .record(TestRecord::new(
            fixture::ROOT,
            "Tag",
            "Tag.BadLayout",
            "synthetic/tags.xml",
        ));
    let (_, catalog) = fixture::diagnostic(&database)?;
    assert!(
        fixture::rows(&catalog, "definitions/tags")?.is_empty(),
        "a failed canonical source produces no fabricated definition"
    );
    let kind = at(&catalog, "/exportHealth/kinds")?
        .as_array()
        .ok_or("health kinds")?
        .iter()
        .find(|kind| kind["kind"] == "tags")
        .ok_or("tag health")?;
    assert_eq!(
        at(kind, "/sourceRecords/omittedAfterReadFailure")?,
        &json!(1)
    );
    assert_eq!(at(kind, "/writtenRows")?, &json!(0));
    assert_eq!(at(&catalog, "/exportHealth/status")?, &json!("incomplete"));
    Ok(())
}

#[test]
fn full_depth_name_and_path_preserve_raw_and_selected_records() -> TestResult {
    for (count, name, former_warning) in [
        (66_u32, "x".to_owned(), "tag_path_depth_limit"),
        (2, "x".repeat(257), "tag_path_name_bytes_limit"),
        (7, "x".repeat(200), "tag_path_bytes_limit"),
    ] {
        let mut database = fixture::schema();
        for index in 0..count {
            let next = if index + 1 < count {
                vec![0x1000 + u128::from(index + 1)]
            } else {
                Vec::new()
            };
            database = database.record(fixture::tag(
                0x1000 + u128::from(index),
                &name,
                fixture::references(&next),
            ));
        }
        database = database.record(fixture::roots(fixture::references(&[0x1000])));
        let (_, catalog) = fixture::build(
            &database,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            None,
        )?;
        assert_eq!(
            fixture::rows(&catalog, "definitions/tags")?.len(),
            usize::try_from(count)?
        );
        let diagnostics = at(&catalog, "/exportHealth/diagnostics")?
            .as_array()
            .ok_or("diagnostics")?;
        assert!(
            diagnostics
                .iter()
                .all(|row| row["warningCode"] != former_warning),
            "valid original data has no projection cutoff diagnostic"
        );
        let paths = fixture::rows(&catalog, "projections/tagPaths")?;
        assert_eq!(paths.len(), usize::try_from(count)?);
        let last = paths.last().ok_or("the actual last tag path")?;
        assert_eq!(at(last, "/recordIndex/value")?, &json!(count - 1));
        assert_eq!(at(last, "/depth/value")?, &json!(count - 1));
        let text = vec![name.as_str(); usize::try_from(count)?].join(".");
        assert_eq!(at(last, "/path/value")?, &json!(text));
        assert_eq!(at(last, "/path/containsEncryptedSource")?, &json!(false));
    }
    Ok(())
}

#[test]
fn cancellation_returns_the_typed_stop_error_without_a_catalog() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::clean(), DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut options = archive_fixture::options();
    options.cancel = Some(&cancel);
    assert!(
        matches!(
            build_catalog(&archive, &options),
            Err(CatalogError::Cancelled)
        ),
        "cancellation publishes no catalog"
    );
    Ok(())
}

#[test]
fn undeclared_tag_fields_and_an_unreadable_name_do_not_become_stored_empty_values() -> TestResult {
    for unreadable in [false, true] {
        let source = if unreadable {
            TestStruct::new("Tag").field("tagName", TestType::String)
        } else {
            TestStruct::new("Tag")
        };
        let record = TestRecord::new(fixture::ROOT, "Tag", "Tag.State", "synthetic/tags.xml");
        let record = if unreadable {
            record.value(
                "tagName",
                TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec()),
            )
        } else {
            record
        };
        let database = DataForgeBuilder::new()
            .version(8)
            .structure(source)
            .record(record);
        let (_, catalog) = fixture::diagnostic(&database)?;
        let row = fixture::row(&catalog, "definitions/tags", 0)?;
        assert_eq!(
            at(row, "/tagName/state")?,
            &json!(if unreadable { "unreadable" } else { "absent" })
        );
        assert!(
            at(row, "/tagName/value")?.is_null(),
            "missing or unreadable stored text is not a stored empty string"
        );
        assert_eq!(at(row, "/legacyGuid/state")?, &json!("absent"));
        assert_eq!(at(row, "/children/state")?, &json!("absent"));
        let path = fixture::row(&catalog, "projections/tagPaths", 0)?;
        assert_eq!(
            at(path, "/path/value")?,
            &json!(""),
            "explicitly named existing convenience fallback"
        );
        assert_eq!(at(path, "/path/sourceSetComplete")?, &json!(!unreadable));
        if unreadable {
            assert!(
                at(path, "/path/containsEncryptedSource")?.is_null(),
                "an unreadable construction input makes plain origin unproven"
            );
        }
    }
    Ok(())
}

#[test]
fn a_nil_authored_guid_stays_present_with_an_explicit_source_diagnostic() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::schema().record(fixture::tag(
        0,
        "Nil",
        fixture::references(&[]),
    )))?;
    let row = fixture::row(&catalog, "definitions/tags", 0)?;
    assert_eq!(at(row, "/guid/value")?, &json!(fixture::guid(0)));
    assert_eq!(at(&catalog, "/exportHealth/status")?, &json!("incomplete"));
    Ok(())
}

#[test]
fn a_new_zero_valued_declared_tag_field_cannot_silently_pass_full_coverage() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("futureTaxonomyFlag", TestType::UInt32),
        )
        .record(
            TestRecord::new(fixture::ROOT, "Tag", "Tag.Future", "synthetic/tags.xml")
                .value("tagName", TestValue::text("Known"))
                .value("futureTaxonomyFlag", TestValue::UInt32(0)),
        );
    let (_, catalog) = fixture::diagnostic(&database)?;
    assert_eq!(
        at(
            fixture::row(&catalog, "definitions/tags", 0)?,
            "/tagName/value"
        )?,
        &json!("Known")
    );
    let diagnostics = at(&catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    assert!(
        future_field_diagnostic(&catalog, diagnostics)?,
        "zero-valued future declarations cannot silently disappear behind complete health"
    );
    assert_eq!(at(&catalog, "/exportHealth/status")?, &json!("incomplete"));
    Ok(())
}

fn future_field_diagnostic(catalog: &Value, diagnostics: &[Value]) -> TestResult<bool> {
    for diagnostic in diagnostics
        .iter()
        .filter(|row| row["code"] == "unsupportedValue")
    {
        let Some(examples) = diagnostic["examples"].as_array() else {
            continue;
        };
        for example in examples {
            if health_origin_assertions::example_text(catalog, example, "field")?
                == Some(&json!("futureTaxonomyFlag"))
                && health_origin_assertions::example_text(catalog, example, "sourceType")?
                    == Some(&json!("Tag"))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
