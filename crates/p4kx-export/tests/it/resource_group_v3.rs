// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult, at, guid};
use crate::resource_group_v3_fixture as fixture;

#[test]
fn canonical_group_tables_keep_physical_definitions_and_every_authored_membership() -> TestResult {
    let (built, catalog) = fixture::diagnostic(&fixture::graph())?;
    assert!(
        catalog.get("resourceGroups").is_none(),
        "no parallel flat group view"
    );
    let rows = fixture::rows(&catalog, "definitions/resourceGroups")?;
    assert_eq!(
        rows.len(),
        5,
        "duplicate physical GUIDs retain both definitions"
    );
    let indexes: Vec<_> = rows
        .iter()
        .map(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64))
        .collect();
    assert_eq!(
        indexes,
        [Some(1), Some(5), Some(4), Some(6), Some(0)],
        "the canonical ResourceGroup source contract is preserved"
    );
    assert!(
        fixture::rows(&catalog, "declarations/resourceGroupMembers")?
            .iter()
            .all(|row| row.get("physical").is_none()),
        "private sorting identity is never a wire field"
    );
    assert_eq!(
        fixture::rows(&catalog, "declarations/resourceGroupMembers")?.len(),
        5,
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        fixture::rows(&catalog, "projections/resourceGroupParents")?.len(),
        5,
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_definition(&catalog)?;
    assert_members(&catalog)?;
    let summary = built
        .summary()
        .kind(Kind::ResourceGroups)
        .ok_or("group summary")?;
    assert_eq!(
        summary.count, 5,
        "declaration and projection rows do not inflate definition counts"
    );
    Ok(())
}

#[test]
fn selected_parent_choice_keeps_scan_order_cycles_and_original_physical_owner() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::graph())?;
    let child = fixture::row(&catalog, "projections/resourceGroupParents", 1)?;
    assert_eq!(
        at(child, "/parent/value/guid/value")?,
        &json!(guid(fixture::FIRST)),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(child, "/parent/value/targetRecordIndex/value")?,
        &json!(0),
        "the canonical ResourceGroup source contract is preserved"
    );
    let root = fixture::row(&catalog, "projections/resourceGroupParents", 0)?;
    assert_eq!(
        at(root, "/parent/value/guid/value")?,
        &json!(guid(fixture::CHILD)),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(root, "/parent/value/targetRecordIndex/value")?,
        &json!(1),
        "the canonical ResourceGroup source contract is preserved"
    );
    let later = fixture::row(&catalog, "projections/resourceGroupParents", 4)?;
    assert!(
        at(later, "/parent/value")?.is_null(),
        "no parent is a deliberate convenience result"
    );
    Ok(())
}

#[test]
fn owner_filter_keeps_declarations_and_parent_choice_within_the_same_selected_scope() -> TestResult
{
    let filter = Filter::new("included");
    let (built, catalog) = fixture::build(
        &fixture::graph(),
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        Some(&filter),
    )?;
    for dataset in [
        "definitions/resourceGroups",
        "declarations/resourceGroupMembers",
        "projections/resourceGroupParents",
    ] {
        let rows = fixture::rows(&catalog, dataset)?;
        assert_eq!(
            rows.len(),
            4,
            "the off-filter dependency does not add {dataset} rows"
        );
        assert!(
            !rows
                .iter()
                .any(|row| row.pointer("/recordIndex/value") == Some(&json!(6))),
            "the canonical ResourceGroup source contract is preserved"
        );
    }
    assert_eq!(
        at(&catalog, "/completeness/resourceGroups")?,
        &json!({"status":"partial", "filter":"included"}),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert!(
        !built
            .summary()
            .kind(Kind::ResourceGroups)
            .ok_or("groups")?
            .complete,
        "the canonical ResourceGroup source contract is preserved"
    );
    Ok(())
}

#[test]
fn valid_groups_publish_only_canonical_v3_and_schema_rejects_missing_tables() -> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::clean(),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    assert_eq!(
        at(&catalog, "/schemaVersion")?,
        &json!(3),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "the canonical ResourceGroup source contract is preserved"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "actual producer positive control"
    );
    for path in [
        "/definitions/resourceGroups",
        "/declarations/resourceGroupMembers",
        "/projections/resourceGroupParents",
    ] {
        let mut invalid = catalog.clone();
        let (parent, field) = path.rsplit_once('/').ok_or("table path")?;
        invalid
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or("table owner")?
            .remove(field)
            .ok_or("required table")?;
        assert!(
            !validator.is_valid(&invalid),
            "the canonical table {path} is required"
        );
    }
    Ok(())
}

fn assert_definition(catalog: &Value) -> TestResult {
    let group = fixture::row(catalog, "definitions/resourceGroups", 0)?;
    assert_eq!(
        at(group, "/recordName/value")?,
        &json!("ResourceTypeGroup.included_first"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(group, "/defaultThumbnailPath/value")?,
        &json!(""),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(group, "/sourceType/value")?,
        &json!("ResourceTypeGroup"),
        "the canonical ResourceGroup source contract is preserved"
    );
    for field in ["groups", "resources", "parentGuid"] {
        assert!(
            group.get(field).is_none(),
            "{field} has its own canonical owner"
        );
    }
    Ok(())
}

fn assert_members(catalog: &Value) -> TestResult {
    let members = fixture::row(catalog, "declarations/resourceGroupMembers", 0)?;
    assert_source_reference_and_catalog_family(catalog, members)?;
    assert_eq!(
        at(members, "/groups/value")?.as_array().map(Vec::len),
        Some(6),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/0/state")?,
        &json!("null"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/1/value/status")?,
        &json!("present"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/2/value/guid/value")?,
        &json!(guid(fixture::MISSING)),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/2/value/status")?,
        &json!("dangling"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/3/value/guid/value")?,
        at(members, "/groups/value/4/value/guid/value")?,
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/groups/value/5/value/guid/value")?,
        &json!(guid(fixture::FIRST)),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/resources/value/1/state")?,
        &json!("null"),
        "the canonical ResourceGroup source contract is preserved"
    );
    assert_eq!(
        at(members, "/resources/value/0/value/guid/value")?,
        at(members, "/resources/value/2/value/guid/value")?,
        "the canonical ResourceGroup source contract is preserved"
    );
    Ok(())
}

#[test]
fn same_guid_edges_between_different_physical_groups_remain_raw_without_choosing_parent()
-> TestResult {
    let database = fixture::schema()
        .record(fixture::group(
            fixture::CHILD,
            "included_same_a",
            fixture::references(&[fixture::CHILD]),
        ))
        .record(fixture::group(
            fixture::CHILD,
            "included_same_b",
            fixture::references(&[fixture::CHILD]),
        ));
    let (_, catalog) = fixture::diagnostic(&database)?;
    let mut cross_physical = false;
    for index in 0..2_u64 {
        let row = fixture::row(&catalog, "declarations/resourceGroupMembers", index)?;
        let edge = at(row, "/groups/value/0/value")?;
        assert_eq!(
            at(edge, "/guid/value")?,
            &json!(guid(fixture::CHILD)),
            "same-GUID source edges remain authored facts"
        );
        let target = at(edge, "/targetRecordIndex/value")?
            .as_u64()
            .ok_or("original physical target")?;
        cross_physical |= target != index;
        let parent = fixture::row(&catalog, "projections/resourceGroupParents", index)?;
        assert!(
            at(parent, "/parent/value")?.is_null(),
            "the retained policy excludes by equal GUID rather than physical index"
        );
    }
    assert!(
        cross_physical,
        "a retained edge identifies a different physical definition"
    );
    Ok(())
}

fn assert_source_reference_and_catalog_family(catalog: &Value, members: &Value) -> TestResult {
    let reference = at(members, "/groups/value/1/value")?;
    let expected_guid = guid(fixture::OTHER);
    assert_eq!(
        at(reference, "/guid/value")?,
        &json!(expected_guid.as_str()),
        "the original stored GUID remains present"
    );
    assert_eq!(
        at(reference, "/targetRecordIndex/value")?,
        &json!(2),
        "the actual Other Record is resolved physically"
    );
    assert_eq!(
        at(reference, "/recordPath/value")?,
        &json!("synthetic/other"),
        "the original target path remains available"
    );
    assert_eq!(
        at(reference, "/targetType/value")?,
        &json!("Other"),
        "canonical source resolution does not impose a catalog-family policy"
    );
    let references = at(catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("catalog reference health")?;
    assert!(
        references.iter().any(|reference| {
            reference.get("recordIndex").and_then(Value::as_u64) == Some(0)
                && reference.get("kind").and_then(Value::as_str) == Some("resourceGroups")
                && reference.get("sourceRole").and_then(Value::as_str) == Some("selected")
                && reference.get("field").and_then(Value::as_str) == Some("groups[1]")
                && reference.get("targetGuid").and_then(Value::as_str)
                    == Some(expected_guid.as_str())
                && reference.get("targetRecordIndex").and_then(Value::as_u64) == Some(2)
                && reference.get("targetRecordPath").and_then(Value::as_str)
                    == Some("synthetic/other")
                && reference.get("targetType").and_then(Value::as_str) == Some("Other")
                && reference.get("status").and_then(Value::as_str) == Some("unexpectedTargetType")
        }),
        "the separate catalog-family health rule rejects the actual wrong-family relationship"
    );
    Ok(())
}
