// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Commodities and blueprints of the Basetool catalog, through the public API.

use crate::catalog_crafting_build;
use crate::catalog_crafting_support::commodity::{self, TextDomain};

use std::error::Error;

use p4kx_export::{Filter, Kind};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn class_names<'a>(catalog: &'a Value, kind: &str) -> Vec<&'a str> {
    catalog[kind]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry["className"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn blueprints_are_selected_by_prefix_and_type_without_leftovers_ordered_by_guid() -> TestResult {
    let (built, catalog) = catalog_crafting_build::build(None)?;
    assert_eq!(
        class_names(&catalog, "blueprints"),
        [
            "BP_CRAFT_behr_lmg_01_mag",
            "BP_CRAFT_core",
            "BP_CRAFT_dangling",
            "BP_CRAFT_weapon",
            "BP_CRAFT_nested",
        ],
        "crafting blueprints incl. $templates, without the dismantle record and the leftover"
    );
    let kind = built.summary().kind(Kind::Blueprints);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((5, 1, true)),
        "counts"
    );
    Ok(())
}

#[test]
fn a_blueprint_carries_key_category_product_and_craft_time() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let blueprint = &catalog["blueprints"][1];
    assert_eq!(
        (
            &blueprint["guid"],
            &blueprint["className"],
            &blueprint["recordPath"],
            &blueprint["key"],
        ),
        (
            &json!("12000000-0000-4000-8000-000000000002"),
            &json!("BP_CRAFT_core"),
            &json!(
                "libs/foundry/records/crafting/blueprints/crafting/vehiclegear/powerplant/bp_craft_core"
            ),
            &json!("BP_CRAFT_core"),
        ),
        "identity"
    );
    assert_eq!(
        blueprint["name"],
        json!({"key": "@LOC_PLACEHOLDER", "en": null, "de": null}),
        "placeholder name"
    );
    assert_eq!(
        blueprint["categoryGuid"], "d1000000-0000-4000-8000-000000000001",
        "category"
    );
    assert_eq!(
        blueprint["producedItemGuid"], "e2000000-0000-4000-8000-000000000002",
        "product"
    );
    assert_eq!(
        blueprint["craftTimeSeconds"], 93_784,
        "1 d 2 h 3 min and 3.6 s, rounded"
    );
    Ok(())
}

#[test]
fn ingredients_with_resources_and_items_keep_quantity_quality_and_slot() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    assert_eq!(
        catalog["blueprints"][0]["ingredients"],
        json!([
            {
                "resourceGuid": "b1000000-0000-4000-8000-000000000001", "itemGuid": null,
                "quantityScu": 0.5, "quantityUnits": null, "minQuality": 0,
                "slot": "MAGAZINE", "slotKey": "@crafting_ui_slotname_magazine",
            },
            {
                "resourceGuid": null, "itemGuid": "e2000000-0000-4000-8000-000000000002",
                "quantityScu": null, "quantityUnits": 5, "minQuality": 500,
                "slot": "CORE", "slotKey": null,
            },
            {
                "resourceGuid": "b2000000-0000-4000-8000-000000000002", "itemGuid": null,
                "quantityScu": 0.25, "quantityUnits": null, "minQuality": 300,
                "slot": "CORE", "slotKey": null,
            },
            {
                "resourceGuid": "b2000000-0000-4000-8000-000000000002", "itemGuid": null,
                "quantityScu": 2.0, "quantityUnits": null, "minQuality": 100,
                "slot": "ASPECTS", "slotKey": null,
            },
        ]),
        "selections are flattened depth first, each line with its nearest slot"
    );
    assert_eq!(
        catalog["blueprints"][1]["ingredients"][0]["slot"],
        Value::Null,
        "a cost outside any selection has no slot"
    );
    Ok(())
}

#[test]
fn a_slot_is_inherited_through_unnamed_nested_selections() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let nested = &catalog["blueprints"][4];
    assert_eq!(nested["className"], "BP_CRAFT_nested", "the blueprint");
    assert_eq!(
        nested["ingredients"][0],
        json!({
            "resourceGuid": "b1000000-0000-4000-8000-000000000001", "itemGuid": null,
            "quantityScu": 3.0, "quantityUnits": null, "minQuality": 200,
            "slot": "OUTER", "slotKey": "@crafting_ui_slotname_outer",
        }),
        "two unnamed selections down, the slot of the named one"
    );
    Ok(())
}

#[test]
fn optional_costs_follow_the_mandatory_ones() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let ingredients = &catalog["blueprints"][4]["ingredients"];
    assert_eq!(
        ingredients.as_array().map(Vec::len),
        Some(3),
        "one mandatory and two optional lines"
    );
    assert_eq!(
        ingredients[1],
        json!({
            "resourceGuid": "b2000000-0000-4000-8000-000000000002", "itemGuid": null,
            "quantityScu": 0.75, "quantityUnits": null, "minQuality": 400,
            "slot": null, "slotKey": null,
        }),
        "an optional resource outside any selection"
    );
    assert_eq!(
        (&ingredients[2]["itemGuid"], &ingredients[2]["slot"]),
        (
            &json!("e2000000-0000-4000-8000-000000000002"),
            &json!("EXTRA")
        ),
        "an optional item in a selection"
    );
    Ok(())
}

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
    let blueprints = catalog["blueprints"].as_array().ok_or("blueprints")?;
    for ingredient in blueprints
        .iter()
        .flat_map(|blueprint| blueprint["ingredients"].as_array().into_iter().flatten())
        .filter(|ingredient| !ingredient["resourceGuid"].is_null())
    {
        let joined = subtypes
            .iter()
            .filter(|subtype| **subtype == &ingredient["resourceGuid"])
            .count();
        assert_eq!(joined, 1, "{}", ingredient["resourceGuid"]);
    }
    Ok(())
}

#[test]
fn a_product_the_database_lacks_is_null() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    assert_eq!(
        catalog["blueprints"][2]["producedItemGuid"],
        Value::Null,
        "dangling Reference"
    );
    assert_eq!(
        catalog["blueprints"][2]["ingredients"][0]["itemGuid"],
        "e2000000-0000-4000-8000-000000000002",
        "the rest of the blueprint is read"
    );
    Ok(())
}

#[test]
fn a_filter_marks_both_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/metals/*");
    let (built, catalog) = catalog_crafting_build::build(Some(&filter))?;
    assert_eq!(commodity::class_names(&catalog)?, ["gold"], "filtered");
    assert!(class_names(&catalog, "blueprints").is_empty(), "filtered");
    for kind in ["commodities", "blueprints"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/metals/*"}),
            "{kind} is partial"
        );
    }
    assert!(
        built.summary().kinds().iter().all(|kind| !kind.complete),
        "the summary agrees"
    );
    Ok(())
}

#[test]
fn without_a_filter_both_kinds_are_complete() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    for kind in ["commodities", "blueprints"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "complete"}),
            "{kind} is complete"
        );
    }
    Ok(())
}

#[test]
fn the_blueprint_kind_selects_crafting_blueprint_records() {
    assert_eq!(
        Kind::Blueprints.rule().map(|rule| rule.type_name),
        Some("CraftingBlueprintRecord"),
        "type"
    );
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
