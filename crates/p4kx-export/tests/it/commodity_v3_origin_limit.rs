// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Kind};
use serde_json::{Value, json};

use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult};
use crate::commodity_v3_origin_limit_fixture::{self as limit_fixture, COST_COUNT};
use crate::commodity_v3_origin_payload as payload;

#[test]
fn wide_blueprint_backfill_keeps_every_original_selection_use() -> TestResult {
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
                "the same logical Commodities are complete"
            );
            payload::assert_full_costs(&catalog)?;
            assert_complete_selection(&catalog, encrypted)?;
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

fn assert_complete_selection(catalog: &Value, encrypted: bool) -> TestResult {
    let row = fixture::definition(catalog, fixture::RESOURCE_FALLBACK)?;
    let selection = fixture::at(row, "/selection")?;
    assert_eq!(
        selection["state"], "present",
        "the fallback selection is present"
    );
    assert_eq!(
        selection["value"], "resourceFallback",
        "the Commodity records its resource fallback"
    );
    assert_eq!(
        selection["sourceSetComplete"], true,
        "every actual fallback selector is retained"
    );
    assert_eq!(
        selection["containsEncryptedSource"], encrypted,
        "fallback selection retains the actual encryption state"
    );
    assert_eq!(
        fixture::at(row, "/guid/containsEncryptedSource")?,
        encrypted,
        "the GUID retains the original encryption state"
    );
    assert_eq!(
        fixture::at(row, "/guid/sourceSetComplete")?,
        true,
        "the GUID has complete provenance"
    );
    assert!(
        row.get("selectionDiagnostics").is_none(),
        "the retired quota field is absent from the definition"
    );
    assert_retained_inputs(catalog, selection)?;
    assert_independent_selection(catalog, encrypted)?;
    assert_exact_commodity_rows(catalog)
}

fn assert_retained_inputs(catalog: &Value, selection: &Value) -> TestResult {
    let recipe = blueprint::by_guid(catalog, limit_fixture::BLUEPRINT)?;
    let record_index = recipe
        .definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical Blueprint index")?;
    assert_eq!(
        record_index,
        limit_fixture::BLUEPRINT_INDEX,
        "the selector belongs to the actual Blueprint Record"
    );
    let retained = blueprint::selected_ordinals(catalog, selection, record_index)?;
    let tiers = recipe.tiers()?;
    let [tier] = tiers.as_slice() else {
        return Err("one cost-bearing tier".into());
    };
    let costs = tier["costs"].as_array().ok_or("cost nodes")?;
    assert_eq!(
        costs.len(),
        usize::from(COST_COUNT),
        "every authored optional cost is retained"
    );
    for cost in costs {
        let original = recipe.node(&cost["source"])?;
        let leaf = blueprint::field_ref(original, "resource")?;
        let ordinal = leaf["valueIndex"]
            .as_str()
            .ok_or("actual resource leaf ordinal")?;
        assert!(
            retained.contains(ordinal),
            "every authored resource slot contributes exactly"
        );
    }
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
        "another Blueprint does not taint an independent resource"
    );
    assert_eq!(
        fixture::at(row, "/selection/containsEncryptedSource")?,
        &json!(encrypted),
        "the unrelated resource keeps only its actual encryption evidence"
    );
    assert!(
        row.get("selectionDiagnostics").is_none(),
        "the unrelated resource uses the same complete selection contract"
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
            assert_row_completeness(row, path == "/definitions/commodities/rows");
        }
    }
    Ok(())
}

fn assert_row_completeness(row: &Value, definition: bool) {
    let mut incomplete = Vec::new();
    incomplete_paths(row, "", &mut incomplete);
    assert!(
        incomplete.is_empty(),
        "all original contributor sets remain complete"
    );
    if definition {
        assert!(
            row.get("selectionDiagnostics").is_none(),
            "the canonical definition excludes the retired quota field"
        );
    }
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
