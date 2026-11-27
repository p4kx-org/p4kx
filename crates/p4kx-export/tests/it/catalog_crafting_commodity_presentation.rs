// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Commodity presentation and the `ResourceType` links used by crafting ingredients.

use serde_json::{Value, json};

use crate::catalog_crafting_build;
use crate::catalog_crafting_support::blueprint;
use crate::catalog_crafting_support::commodity::{self, TextDomain};

use super::TestResult;

#[test]
fn a_placeholder_name_and_description_fall_back_to_the_resource_type() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let definition = &commodity::definitions(&catalog)?[2];
    let presentation = &commodity::presentations(&catalog)?[2];
    assert_eq!(definition["className"]["value"], "ember", "the commodity");
    commodity::assert_text(
        &catalog,
        &presentation["name"],
        TextDomain::PublicName,
        Some("@items_commodities_ember"),
        Some("Ember"),
        None,
    )?;
    commodity::assert_text(
        &catalog,
        &presentation["description"],
        TextDomain::GameText,
        Some("@items_commodities_ember_desc"),
        Some("A glowing mineral."),
        None,
    )?;
    Ok(())
}

#[test]
fn a_commodity_carries_the_density_and_the_refined_version_of_its_resource_type() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let ember = &commodity::presentations(&catalog)?[2];
    assert_eq!(
        ember["densityGramsPerCubicCentimetre"]["value"], 2.5,
        "density"
    );
    assert_eq!(
        ember["refinedSubtypeGuid"]["value"], "b4000000-0000-4000-8000-000000000004",
        "the refined ResourceType"
    );
    assert_eq!(
        commodity::presentations(&catalog)?[1]["densityGramsPerCubicCentimetre"]["value"],
        Value::Null,
        "no ResourceType, no density"
    );
    Ok(())
}

#[test]
fn a_used_resource_type_without_a_commodity_record_is_a_commodity_of_its_group() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let definition = commodity::definitions(&catalog)?
        .get(3)
        .ok_or("the fourth Commodity is the resource-only fallback")?;
    let presentation = commodity::presentations(&catalog)?
        .get(3)
        .ok_or("the fourth Commodity has a canonical presentation")?;
    assert_eq!(
        (
            &definition["guid"]["value"],
            &definition["className"]["value"],
            &definition["recordPath"]["value"]
        ),
        (
            &json!("b2000000-0000-4000-8000-000000000002"),
            &json!("iron"),
            &json!("libs/foundry/records/resourcetypedatabase/resourcetypedatabase")
        )
    );
    assert_eq!(definition["selection"]["value"], "resourceFallback");
    assert_eq!(
        presentation["commodityGuid"]["value"],
        definition["guid"]["value"]
    );
    assert_eq!(
        (
            &presentation["typeGuid"]["value"],
            &presentation["subtypeGuid"]["value"],
            &presentation["densityGramsPerCubicCentimetre"]["value"],
            &presentation["refinedSubtypeGuid"]["value"]
        ),
        (
            &json!("c1000000-0000-4000-8000-000000000001"),
            &json!("b2000000-0000-4000-8000-000000000002"),
            &json!(7.5),
            &json!("b1000000-0000-4000-8000-000000000001")
        )
    );
    assert_backfilled_iron_texts(&catalog, presentation)?;
    assert!(
        !commodity::class_names(&catalog)?.contains(&"copper"),
        "an unused ResourceType stays out"
    );
    Ok(())
}

#[test]
fn every_resource_ingredient_joins_exactly_one_commodity() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let subtypes: Vec<&Value> = commodity::presentations(&catalog)?
        .iter()
        .map(|presentation| &presentation["subtypeGuid"]["value"])
        .collect();
    for index in 0..blueprint::definitions(&catalog)?.len() {
        let blueprint = blueprint::by_index(&catalog, index)?;
        for tier in blueprint.tiers()? {
            for cost in tier["costs"].as_array().ok_or("interpreted costs")? {
                let resource = &cost["meaning"]["resourceGuid"]["value"];
                if !resource.is_null() {
                    assert_eq!(
                        subtypes
                            .iter()
                            .filter(|subtype| **subtype == resource)
                            .count(),
                        1,
                        "every resource in all tiers, including nested options, resolves one Commodity"
                    );
                }
            }
        }
    }
    Ok(())
}

fn assert_backfilled_iron_texts(catalog: &Value, presentation: &Value) -> TestResult {
    commodity::assert_text(
        catalog,
        presentation.get("name").ok_or("backfilled name carrier")?,
        TextDomain::PublicName,
        Some("@items_commodities_iron"),
        None,
        None,
    )?;
    commodity::assert_text(
        catalog,
        presentation
            .get("description")
            .ok_or("backfilled description carrier")?,
        TextDomain::GameText,
        Some("@items_commodities_iron_desc"),
        None,
        None,
    )?;
    commodity::assert_text(
        catalog,
        presentation
            .get("displayType")
            .ok_or("backfilled display-type carrier")?,
        TextDomain::GameText,
        None,
        None,
        None,
    )?;
    Ok(())
}
