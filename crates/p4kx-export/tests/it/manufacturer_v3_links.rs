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
    let rows = fixture::rows(&catalog)?;
    let [definition] = rows.as_slice() else {
        return Err("exactly one selected manufacturer definition".into());
    };
    let authored = authored_manufacturer(&catalog)?;
    assert_eq!(
        fixture::at(authored, "/guid/value")?,
        fixture::at(definition, "/guid/value")?,
        "the authored identity joins the selected definition"
    );
    assert_eq!(
        fixture::at(authored, "/targetRecordIndex/value")?,
        fixture::at(definition, "/recordIndex/value")?,
        "the authored Reference joins the same physical definition"
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
        let authored = authored_manufacturer(&catalog)?;
        let index = fixture::at(authored, "/targetRecordIndex/value")?;
        assert!(
            references(&catalog)?.iter().any(|reference| {
                reference.get("status").and_then(Value::as_str) == Some(expected)
                    && reference.pointer("/targetRecordIndex/value") == Some(index)
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
        matches!(normal, Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport(_))))
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

fn authored_manufacturer(catalog: &Value) -> TestResult<&Value> {
    let projection = fixture::at(catalog, "/items/0")?;
    let attachment = crate::catalog_items::selected_attachment(catalog, projection)?;
    assert_eq!(
        fixture::at(attachment, "/manufacturer/kind")?,
        &json!("record"),
        "the selected attachment contains an original Record Reference"
    );
    let link = fixture::at(attachment, "/manufacturer/value/value")?;
    assert_eq!(
        fixture::at(link, "/guid/value")?,
        &json!(fixture::guid(fixture::MAKER)),
        "the authored manufacturer GUID survives"
    );
    assert_eq!(
        fixture::at(link, "/targetRecordIndex/value")?,
        &json!(0),
        "the actual original target survives catalog selection"
    );
    assert_eq!(
        fixture::at(link, "/status")?,
        &json!("present"),
        "the DataForge Reference resolves in the source"
    );
    Ok(link)
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
        matches!(normal, Err(error) if matches!(error.downcast_ref::<CatalogError>(), Some(CatalogError::IncompleteExport(_))))
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
    let mut found = false;
    for diagnostic in diagnostics {
        if diagnostic.get("code").and_then(Value::as_str) != Some("malformedSource") {
            continue;
        }
        let examples = diagnostic["examples"]
            .as_array()
            .ok_or("identity examples")?;
        for example in examples {
            found |= crate::health_origin_assertions::example_text(&catalog, example, "field")?
                == Some(&json!("$guid"));
        }
    }
    assert!(
        found,
        "the actual invalid identity has its typed source location"
    );
    Ok(())
}
