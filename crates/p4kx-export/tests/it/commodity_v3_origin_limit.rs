// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Kind};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult};
use crate::commodity_v3_origin_limit_fixture::{self as limit_fixture, COST_COUNT};
use crate::commodity_v3_origin_payload as payload;
use crate::commodity_v3_source;
use crate::commodity_v3_source_inventory::SourceField;

#[test]
fn capture_limit_keeps_payload_and_publication_complete() -> TestResult {
    let database = limit_fixture::database();
    let mut plain = None;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = fixture::open(&database, origin)?;
        let mut normal = None;
        for (mode, label) in [
            (CatalogPublicationMode::Normal, "normal"),
            (
                CatalogPublicationMode::DiagnosticPartial,
                "diagnosticPartial",
            ),
        ] {
            let mut options = fixture::options();
            options.publication_mode = mode;
            let built = p4kx_export::build_catalog(&archive, &options)?;
            let catalog: Value = serde_json::from_slice(built.json())?;
            payload::assert_complete(&catalog)?;
            assert_eq!(catalog["exportHealth"]["publicationMode"], label);
            assert_eq!(
                built
                    .summary()
                    .kind(Kind::Commodities)
                    .map(|kind| (kind.count, kind.complete)),
                Some((5, true)),
                "origin metadata loss cannot omit a logical Commodity"
            );
            payload::assert_full_costs(&catalog)?;
            assert_selection_only_limit(&catalog, encrypted)?;
            if let Some(first) = &normal {
                payload::same_payload(first, &catalog, "")?;
            } else {
                normal = Some(catalog);
            }
        }
        let normal = normal.ok_or("normal product build missing")?;
        if let Some(first) = &plain {
            payload::same_payload(first, &normal, "")?;
        } else {
            plain = Some(normal);
        }
    }
    Ok(())
}

fn assert_selection_only_limit(catalog: &Value, encrypted: bool) -> TestResult {
    let expected = if encrypted {
        Value::Bool(true)
    } else {
        Value::Null
    };
    let row = fixture::definition(catalog, fixture::RESOURCE_FALLBACK)?;
    assert_eq!(
        fixture::at(row, "/selection/state")?,
        &json!("present"),
        "the source selection remains a present choice"
    );
    assert_eq!(
        fixture::at(row, "/selection/value")?,
        &json!("resourceFallback"),
        "the source selection keeps its fallback kind"
    );
    assert_eq!(
        fixture::at(row, "/selection/sourceSetComplete")?,
        &json!(false),
        "the capture limit marks only selection contributors incomplete"
    );
    assert_eq!(
        fixture::at(row, "/selection/containsEncryptedSource")?,
        &expected,
        "known encryption stays true while incomplete plain lineage stays unknown"
    );
    assert_eq!(
        fixture::at(row, "/guid/containsEncryptedSource")?,
        &json!(encrypted),
        "the raw GUID has its own complete physical source evidence"
    );
    assert_eq!(
        fixture::at(row, "/guid/sourceSetComplete")?,
        &json!(true),
        "the independent raw GUID contributor set remains complete"
    );
    assert_limit_diagnostic(row)?;
    assert_retained_inputs(catalog, row)?;
    assert_independent_selection(catalog, encrypted)?;
    assert_exact_commodity_rows(catalog)
}

fn assert_limit_diagnostic(row: &Value) -> TestResult {
    let diagnostics = fixture::at(row, "/selectionDiagnostics")?
        .as_array()
        .ok_or("selection diagnostics")?;
    assert_eq!(
        diagnostics.len(),
        1,
        "one bounded origin metadata limit is reported"
    );
    let diagnostic = diagnostics.first().ok_or("origin metadata observation")?;
    assert_eq!(
        fixture::at(diagnostic, "/code")?,
        &json!("originMetadataLimit"),
        "the report names the actual metadata capture limit"
    );
    assert_eq!(
        fixture::at(diagnostic, "/recordIndex/value")?,
        &json!(limit_fixture::BLUEPRINT_INDEX),
        "the report retains the actual limiting Blueprint owner"
    );
    assert_eq!(
        fixture::at(diagnostic, "/recordIndex/sourceSetComplete")?,
        &json!(true),
        "the diagnostic owner has complete physical evidence"
    );
    Ok(())
}

fn assert_retained_inputs(catalog: &Value, row: &Value) -> TestResult {
    let uses = commodity_v3_source::uses(catalog, fixture::at(row, "/selection")?)?;
    let retained = uses
        .iter()
        .filter(|(index, path, role)| {
            *index == limit_fixture::BLUEPRINT_INDEX
                && matches!(path, SourceField::Body(path) if path.starts_with("blueprint.tiers[0].recipe.costs.optionalCosts[") && path.ends_with(".resource"))
                && role == "selector"
        })
        .count();
    assert!(
        (1..usize::from(COST_COUNT)).contains(&retained),
        "the production capture retains real distinct inputs and reports the remaining gap"
    );
    Ok(())
}

fn assert_independent_selection(catalog: &Value, encrypted: bool) -> TestResult {
    let row = fixture::definition(catalog, limit_fixture::INDEPENDENT_RESOURCE)?;
    assert_eq!(
        fixture::at(row, "/selection/value")?,
        &json!("resourceFallback"),
        "the unrelated resource keeps its fallback selection"
    );
    assert_eq!(
        fixture::at(row, "/selection/sourceSetComplete")?,
        &json!(true),
        "an unrelated Blueprint capture limit cannot taint another resource"
    );
    assert_eq!(
        fixture::at(row, "/selection/containsEncryptedSource")?,
        &json!(encrypted),
        "the unrelated resource keeps only its actual encryption evidence"
    );
    assert_eq!(
        fixture::at(row, "/selectionDiagnostics")?
            .as_array()
            .map(Vec::len),
        Some(0),
        "the unrelated resource gains no metadata-limit diagnostic"
    );
    Ok(())
}

fn assert_exact_commodity_rows(catalog: &Value) -> TestResult {
    let expected = [
        fixture::RESOURCE_FALLBACK,
        limit_fixture::INDEPENDENT_RESOURCE,
        fixture::ALPHA,
        fixture::BETA,
        fixture::UNLINKED,
    ]
    .map(fixture::guid);
    for (path, identity) in [
        ("/definitions/commodities/rows", "guid"),
        ("/projections/commodityPresentation/rows", "commodityGuid"),
    ] {
        let rows = fixture::at(catalog, path)?
            .as_array()
            .ok_or("canonical rows")?;
        assert_eq!(
            rows.len(),
            expected.len(),
            "origin capture cannot change either canonical row count"
        );
        for (row, guid) in rows.iter().zip(&expected) {
            assert_eq!(
                fixture::at(row, &format!("/{identity}/value"))?.as_str(),
                Some(guid.as_str()),
                "row identities and GUID order survive"
            );
            let limited = path == "/definitions/commodities/rows"
                && fixture::at(row, "/guid/value")?.as_str()
                    == Some(fixture::guid(fixture::RESOURCE_FALLBACK).as_str());
            assert_row_completeness(row, limited, path == "/definitions/commodities/rows")?;
        }
    }
    Ok(())
}

fn assert_row_completeness(row: &Value, limited: bool, definition: bool) -> TestResult {
    let mut incomplete = Vec::new();
    incomplete_paths(row, "", &mut incomplete);
    let expected_incomplete = if limited {
        vec!["/selection"]
    } else {
        Vec::new()
    };
    assert_eq!(
        incomplete, expected_incomplete,
        "only the selection's contributor completeness changes"
    );
    if !limited && definition {
        assert_eq!(
            fixture::at(row, "/selectionDiagnostics")?
                .as_array()
                .map(Vec::len),
            Some(0),
            "other definitions gain no capture-limit diagnosis"
        );
    }
    Ok(())
}
fn incomplete_paths(value: &Value, path: &str, result: &mut Vec<String>) {
    if value.get("sourceSetComplete") == Some(&Value::Bool(false)) {
        result.push(path.to_owned());
    }
    match value {
        Value::Object(object) => {
            for (name, child) in object {
                incomplete_paths(child, &format!("{path}/{name}"), result);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                incomplete_paths(child, &format!("{path}/{index}"), result);
            }
        }
        _ => {}
    }
}
