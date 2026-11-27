// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resource source coverage and catalog materialization remain independent.

use std::error::Error;

use p4kx_export::{CatalogError, Filter};
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::quality_v3_support as source;
use crate::resource_reference_fixture::{self as fixture, TargetShape};

type TestResult = Result<(), Box<dyn Error>>;

fn references(catalog: &Value) -> Result<&Vec<Value>, Box<dyn Error>> {
    catalog
        .pointer("/exportHealth/unresolvedReferences")
        .and_then(Value::as_array)
        .ok_or_else(|| "the unresolved references".into())
}

#[test]
fn a_policy_excluded_quality_target_is_retained_without_claiming_a_row() -> TestResult {
    let (_, catalog) =
        catalog_crafting_support::build_from(&fixture::target(TargetShape::Leftover), None)?;
    assert_eq!(source::rows(&catalog, source::DISTRIBUTIONS)?.len(), 0);
    let source = &catalog["resourceTypes"][0]["properties"][0]["payload"]["qualityDistribution"];
    assert_eq!(source["payload"]["reference"]["status"], "present");
    assert_eq!(source["payload"]["reference"]["targetRecordIndex"], 1);
    assert!(
        references(&catalog)?
            .iter()
            .any(|reference| reference["status"] == "excludedByPolicy"
                && reference["targetRecordIndex"]["value"] == 1)
    );
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

#[test]
fn a_filtered_quality_target_is_distinct_from_a_missing_source() -> TestResult {
    let filter = Filter::new("included");
    let (_, catalog) =
        catalog_crafting_support::build_from(&fixture::target(TargetShape::Kept), Some(&filter))?;
    assert_eq!(source::rows(&catalog, source::DISTRIBUTIONS)?.len(), 0);
    assert!(
        references(&catalog)?
            .iter()
            .any(|reference| reference["status"] == "excludedByFilter"
                && reference["targetRecordIndex"]["value"] == 1)
    );
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

#[test]
fn a_retained_quality_field_violation_does_not_become_an_omitted_reference_target() -> TestResult {
    let (_, catalog) = catalog_crafting_support::diagnostic_from(
        &fixture::target(TargetShape::InexactBounds),
        None,
    )?;
    let quality = source::row(&catalog, source::DISTRIBUTIONS, 1)?;
    assert_eq!(
        source::at(
            quality,
            "/qualityDistribution/value/payload/value/value/value/min/value/type"
        )?,
        &json!("f32")
    );
    source::assert_field_error(
        &catalog,
        "qualityDistribution.min",
        "CraftingQualityDistributionNormal",
    )?;
    assert_eq!(catalog["resourceTypes"].as_array().map(Vec::len), Some(1));
    assert!(
        !references(&catalog)?
            .iter()
            .any(|reference| reference["targetRecordIndex"]["value"] == 1),
        "the readable canonical raw target is materialized despite its separate field violation"
    );
    assert_eq!(catalog["exportHealth"]["status"], "incomplete");
    Ok(())
}

#[test]
fn a_wrong_quality_target_family_is_not_accepted_as_a_present_catalog_definition() -> TestResult {
    let database = fixture::target(TargetShape::WrongFamily);
    let Err(error) = catalog_crafting_support::build_from(&database, None) else {
        return Err("the wrong target family must block normal publication".into());
    };
    assert!(matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport(_))
    ));
    let (_, catalog) = catalog_crafting_support::diagnostic_from(&database, None)?;
    assert!(
        references(&catalog)?
            .iter()
            .any(|reference| reference["status"] == "unexpectedTargetType"
                && reference["targetRecordIndex"]["value"] == 0)
    );
    Ok(())
}

#[test]
fn an_unknown_property_keeps_its_source_shape_and_explicit_unsupported_health() -> TestResult {
    let (_, catalog) =
        catalog_crafting_support::diagnostic_from(&fixture::unknown_property(), None)?;
    let property = &catalog["resourceTypes"][0]["properties"][0];
    assert_eq!(property["sourceType"], "FixtureFutureProperty");
    assert_eq!(property["name"], "future");
    assert_eq!(property["payload"], json!({"type": "unsupported"}));
    let diagnostics = catalog["exportHealth"]["diagnostics"]
        .as_array()
        .ok_or("the diagnostics")?;
    let mut found = false;
    for diagnostic in diagnostics {
        if diagnostic["code"] != "unsupportedValue" {
            continue;
        }
        let examples = diagnostic["examples"]
            .as_array()
            .ok_or("actual unsupported examples")?;
        for example in examples {
            if example["field"]["state"] == "present" && example["sourceType"]["state"] == "present"
            {
                found |= crate::health_origin_assertions::text_value(
                    &catalog["exportHealth"],
                    &example["field"],
                )? == &json!("properties[0]")
                    && crate::health_origin_assertions::text_value(
                        &catalog["exportHealth"],
                        &example["sourceType"],
                    )? == &json!("FixtureFutureProperty");
            }
        }
    }
    assert!(
        found,
        "the exact unsupported original source type and field path survive canonical pooling"
    );
    assert_eq!(catalog["exportHealth"]["status"], "incomplete");
    Ok(())
}
