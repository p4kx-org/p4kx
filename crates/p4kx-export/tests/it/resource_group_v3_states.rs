// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult, at};
use crate::resource_group_v3_fixture as fixture;

#[test]
fn group_array_source_states_do_not_turn_into_successful_empty_lists() -> TestResult {
    let mut invalid = 1_u32.to_le_bytes().to_vec();
    invalid.extend_from_slice(&u32::MAX.to_le_bytes());
    for (state, declaration, value) in [
        ("absent", None, None),
        (
            "null",
            Some((TestType::StrongPointer("Empty".to_owned()), false)),
            Some(TestValue::Null),
        ),
        (
            "present",
            Some((TestType::ReferenceTo("ResourceTypeGroup".to_owned()), true)),
            Some(fixture::references(&[])),
        ),
        (
            "invalid",
            Some((TestType::Bool, false)),
            Some(TestValue::Bool(false)),
        ),
        (
            "unreadable",
            Some((TestType::ReferenceTo("ResourceTypeGroup".to_owned()), true)),
            Some(TestValue::Raw(invalid)),
        ),
    ] {
        check_array_state(state, declaration, value)?;
    }
    Ok(())
}

fn check_array_state(
    state: &str,
    declaration: Option<(TestType, bool)>,
    value: Option<TestValue>,
) -> TestResult {
    let mut group = TestStruct::new("ResourceTypeGroup");
    if let Some((kind, array)) = declaration {
        group = if array {
            group.array("groups", kind)
        } else {
            group.field("groups", kind)
        };
    }
    let mut record = TestRecord::new(
        fixture::FIRST,
        "ResourceTypeGroup",
        "ResourceTypeGroup.State",
        "libs/foundry/records/resourcetypedatabase/state.xml",
    );
    if let Some(value) = value {
        record = record.value("groups", value);
    }
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(group)
        .structure(TestStruct::new("Empty"))
        .record(record);
    let (_, catalog) = fixture::diagnostic(&database)?;
    let row = fixture::row(&catalog, "declarations/resourceGroupMembers", 0)?;
    assert_eq!(
        at(row, "/groups/state")?,
        &json!(state),
        "the authored source state stays distinct"
    );
    assert_eq!(
        at(row, "/groups/value")?,
        &if state == "present" {
            json!([])
        } else {
            json!(null)
        },
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        fixture::rows(&catalog, "definitions/resourceGroups")?.len(),
        1,
        "readable scalar facts survive an invalid membership field"
    );
    if matches!(state, "invalid" | "unreadable") {
        assert!(
            matches!(fixture::build(&database, DcbOrigin::Plain, CatalogPublicationMode::Normal, None), Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport { .. }))),
            "normal publication retains its typed refusal"
        );
    }
    Ok(())
}

#[test]
fn unknown_group_fields_block_full_coverage_even_with_a_zero_value() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ResourceTypeGroup").field("futureGroupControl", TestType::UInt32),
        )
        .record(
            TestRecord::new(
                fixture::FIRST,
                "ResourceTypeGroup",
                "ResourceTypeGroup.Future",
                "libs/foundry/records/resourcetypedatabase/future.xml",
            )
            .value("futureGroupControl", TestValue::UInt32(0)),
        );
    let (_, catalog) = fixture::diagnostic(&database)?;
    assert_eq!(
        fixture::rows(&catalog, "definitions/resourceGroups")?.len(),
        1,
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        &json!("incomplete"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert!(
        matches!(fixture::build(&database, DcbOrigin::Plain, CatalogPublicationMode::Normal, None), Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport { .. }))),
        "unhandled declarations cannot publish normally"
    );
    let diagnostics = at(&catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.get("code").and_then(Value::as_str)
                == Some("unsupportedValue")),
        "coverage names the unsupported field"
    );
    Ok(())
}

#[test]
fn group_source_schema_requires_each_nested_origin_and_closed_resolution_control() -> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::clean(),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let carrier = "/declarations/resourceGroupMembers/rows/1/groups/value/0";
    for field in [
        "state",
        "value",
        "sourceUsesRefs",
        "sourceSetComplete",
        "containsEncryptedSource",
    ] {
        let mut invalid = catalog.clone();
        invalid
            .pointer_mut(carrier)
            .and_then(Value::as_object_mut)
            .ok_or("array element")?
            .remove(field)
            .ok_or("carrier field")?;
        assert!(
            !validator.is_valid(&invalid),
            "required nested origin {field}"
        );
    }
    let mut invalid = catalog;
    *invalid
        .pointer_mut(&format!("{carrier}/value/status"))
        .ok_or("resolution status")? = json!("futureResolution");
    assert!(
        !validator.is_valid(&invalid),
        "source resolution is a closed control"
    );
    Ok(())
}
