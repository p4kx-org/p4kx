// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::{DataForge, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture;
use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source::{self as source, Slot};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::resource_edges_fixture;

#[test]
fn category_lists_v3_preserves_duplicate_physical_owners_without_a_global_guid_waiver() -> TestResult
{
    let database = fixture::clean()
        .record(fixture::listing(
            fixture::LIST,
            "Alias",
            "synthetic/Alias.XML",
            fixture::references(&[fixture::ALPHA]),
        ))
        .record(fixture::category(fixture::ALPHA, "AlphaAlias"));
    source::primary(&database)?;
    source::header(
        &database,
        6,
        fixture::LIST,
        "BlueprintCategoryDatabaseRecord.Alias",
        "synthetic/Alias.XML",
        "AliasDomain",
    )?;
    let original = source::slots(&database, 0)?;
    let Some(Slot::Reference { index, .. }) = original.get(2) else {
        return Err("the original category reference must retain its actual physical alias".into());
    };
    let (built, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )?;
    fixture::count(&built, 3, 1)?;
    assert_duplicate_rows(&catalog, *index)?;
    assert_existing_alias_refusal()?;
    Ok(())
}

fn assert_duplicate_rows(catalog: &Value, target_index: u32) -> TestResult {
    assert_eq!(
        indices(fixture::rows(catalog)?)?,
        [0, 6, 4],
        "equal list GUIDs retain distinct physical owners and original tie order"
    );
    assert_eq!(
        indices(blueprint_category_v3_fixture::rows(catalog)?)?,
        [1, 7, 3],
        "the existing category definition scope retains its physical duplicates"
    );
    assert_eq!(
        fixture::at(
            fixture::row(catalog, 0)?,
            "/categories/value/2/value/targetRecordIndex/value"
        )?,
        &json!(target_index),
        "a source reference is not retargeted by a category catalog join"
    );
    assert_eq!(
        fixture::at(fixture::row(catalog, 6)?, "/metadataTag/value")?,
        &json!("AliasDomain"),
        "same-GUID owner headers are not borrowed from another row"
    );
    Ok(())
}

fn indices(rows: &[Value]) -> TestResult<Vec<u64>> {
    rows.iter()
        .map(|row| {
            fixture::at(row, "/recordIndex/value")?
                .as_u64()
                .ok_or_else(|| "physical index".into())
        })
        .collect()
}

fn assert_existing_alias_refusal() -> TestResult {
    let database =
        resource_edges_fixture::aliases(resource_edges_fixture::LinkStorage::TypedReference);
    let filter = Filter::new("included");
    assert_alias_source(&database, &filter)?;
    fixture::refused(&database, Some(&filter))?;
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        Some(&filter),
    )?;
    let failures = fixture::at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("existing unresolved-reference policy")?;
    assert!(
        failures.iter().any(|row| {
            row.get("status").and_then(Value::as_str) == Some("ambiguousIdentity")
                && row.get("targetRecordIndex").and_then(Value::as_u64) == Some(0)
        }),
        "adding raw category lists cannot waive an existing Resource physical-identity failure"
    );
    Ok(())
}

fn assert_alias_source(database: &DataForgeBuilder, filter: &Filter) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("original Resource alias")?;
    let second = forge.record(1).ok_or("selected Resource alias")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the negative control really has one nominal GUID and two physical Records"
    );
    assert!(
        !filter.matches(first.record_path()?) && filter.matches(second.record_path()?),
        "the source target is excluded while a nominal alias is selected"
    );
    assert_original_alias_target(&forge)?;
    Ok(())
}

fn assert_original_alias_target(forge: &DataForge<'_>) -> TestResult {
    let tree = forge.record(2).ok_or("original Commodity owner")?.tree()?;
    let Some(Raw::Array(components)) = tree.root().field("Components") else {
        return Err("original Components array".into());
    };
    let Some(Raw::Struct(container)) = components.first() else {
        return Err("original ResourceContainer".into());
    };
    let Some(Raw::Array(entries)) = container.field("defaultComposition") else {
        return Err("original composition".into());
    };
    let Some(Raw::Struct(entry)) = entries.first() else {
        return Err("original composition entry".into());
    };
    let Some(Raw::Reference(target)) = entry.field("entry") else {
        return Err("original typed Resource Reference".into());
    };
    assert_eq!(
        target.record_index(),
        0,
        "the actual original Reference names the excluded first alias"
    );
    Ok(())
}
