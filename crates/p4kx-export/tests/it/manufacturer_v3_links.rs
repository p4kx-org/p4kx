// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, Filter};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::manufacturer_v3_fixture::{self as fixture, TestResult};
use crate::manufacturer_v3_link_fixture::{self as links, Target};

#[test]
fn an_authored_item_reference_joins_the_actual_selected_definition() -> TestResult {
    let (_, catalog) = fixture::normal(&links::database(Target::Kept))?;
    assert_eq!(fixture::rows(&catalog)?.len(), 1);
    assert_eq!(
        fixture::at(&catalog, "/items/0/manufacturerGuid")?,
        &json!(fixture::guid(fixture::MAKER))
    );
    assert!(
        references(&catalog)?.is_empty(),
        "the exact emitted physical target resolves"
    );
    Ok(())
}

#[test]
fn excluded_or_outside_manufacturers_remain_authored_links_without_auto_inclusion() -> TestResult {
    for (target, expected) in [
        (Target::Filtered, "excludedByFilter"),
        (Target::Leftover, "excludedByPolicy"),
        (Target::Outside, "outsideCatalogSelection"),
    ] {
        let filter = Filter::new("included");
        let filter = matches!(target, Target::Filtered).then_some(&filter);
        let (_, catalog) = fixture::build(
            &links::database(target),
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            filter,
        )?;
        assert!(
            fixture::rows(&catalog)?.is_empty(),
            "untouched targets do not become definitions"
        );
        assert_eq!(
            fixture::at(&catalog, "/items/0/manufacturerGuid")?,
            &json!(fixture::guid(fixture::MAKER))
        );
        assert!(
            references(&catalog)?.iter().any(|reference| {
                reference.get("status").and_then(Value::as_str) == Some(expected)
                    && reference.get("targetRecordIndex").and_then(Value::as_u64) == Some(0)
            }),
            "{target:?} keeps its exact materialization status"
        );
    }
    Ok(())
}

#[test]
fn equal_guid_aliases_keep_physical_rows_and_cannot_silently_resolve() -> TestResult {
    let database = links::database(Target::Alias);
    let normal = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    );
    assert!(
        matches!(normal, Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport { .. })))
    );
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let rows = fixture::rows(&catalog)?;
    assert_eq!(
        rows.len(),
        2,
        "diagnostic rows preserve both physical sources"
    );
    let indices: Vec<_> = rows
        .iter()
        .map(|row| fixture::at(row, "/recordIndex/value"))
        .collect::<Result<_, _>>()?;
    assert_eq!(indices, vec![&json!(0), &json!(2)]);
    assert!(
        references(&catalog)?
            .iter()
            .any(|reference| reference.get("status").and_then(Value::as_str)
                == Some("ambiguousIdentity"))
    );
    Ok(())
}

fn references(catalog: &Value) -> TestResult<&Vec<Value>> {
    fixture::at(catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or_else(|| "reference health rows".into())
}

#[test]
fn a_nil_authored_identity_is_retained_as_a_raw_fact_and_diagnosed() -> TestResult {
    let database = fixture::database().record(fixture::record(
        0,
        "NilIdentity",
        "libs/foundry/records/scitemmanufacturer/included/nil.xml",
    ));
    let normal = fixture::normal(&database);
    assert!(
        matches!(normal, Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport { .. })))
    );
    let (_, catalog) = fixture::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let row = fixture::rows(&catalog)?
        .first()
        .ok_or("diagnostic nil identity row")?;
    assert_eq!(
        fixture::at(row, "/guid/value")?,
        &json!(fixture::guid(0)),
        "no replacement GUID is invented"
    );
    let diagnostics = fixture::at(&catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("identity diagnostics")?;
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.get("code").and_then(Value::as_str) == Some("malformedSource")
                && diagnostic
                    .get("examples")
                    .and_then(Value::as_array)
                    .is_some_and(|examples| {
                        examples.iter().any(|example| {
                            example.get("field").and_then(Value::as_str) == Some("$guid")
                        })
                    })
        }),
        "the actual invalid identity has its typed source location"
    );
    Ok(())
}
