// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Commodities and blueprints of the Basetool catalog, through the public API.

use crate::catalog_crafting_build;
use crate::catalog_crafting_support::blueprint;
use crate::catalog_crafting_support::commodity;

use std::error::Error;

use p4kx_export::{Filter, Kind};
use serde_json::{Value, json};

#[path = "catalog_crafting_commodity_presentation.rs"]
mod commodity_presentation;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn class_names(catalog: &Value) -> TestResult<Vec<&str>> {
    Ok(blueprint::definitions(catalog)?
        .iter()
        .filter_map(|definition| {
            definition
                .pointer("/className/value")
                .and_then(Value::as_str)
        })
        .collect())
}

#[test]
fn blueprints_are_selected_by_prefix_and_type_without_leftovers_ordered_by_guid() -> TestResult {
    let (built, catalog) = catalog_crafting_build::build(None)?;
    assert_eq!(
        class_names(&catalog)?,
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
    let blueprint = blueprint::by_index(&catalog, 1)?;
    let header = blueprint.definition;
    assert_eq!(
        header["guid"]["value"],
        "12000000-0000-4000-8000-000000000002"
    );
    assert_eq!(header["className"]["value"], "BP_CRAFT_core");
    assert_eq!(
        header["recordName"]["value"],
        "CraftingBlueprintRecord.BP_CRAFT_core"
    );
    assert_eq!(
        header["recordPath"]["value"],
        "libs/foundry/records/crafting/blueprints/crafting/vehiclegear/powerplant/bp_craft_core"
    );
    let original = blueprint.root("blueprint")?;
    let name = blueprint.field(original, "blueprintName")?;
    assert_eq!(
        name["value"]["value"]["value"]["sourceText"], "@LOC_PLACEHOLDER",
        "the original placeholder survives with its source facts"
    );
    assert!(
        blueprint.presentation["name"]["value"].is_null(),
        "a placeholder does not produce a localized display name"
    );
    assert_eq!(
        blueprint.presentation["category"]["value"],
        "d1000000-0000-4000-8000-000000000001"
    );
    assert_eq!(
        blueprint.presentation["producedItem"]["value"],
        "e2000000-0000-4000-8000-000000000002"
    );
    let tier = blueprint.tiers()?.first().copied().ok_or("first tier")?;
    assert_eq!(tier["source"], blueprint.presentation["firstTier"]);
    assert_eq!(
        tier["craftTimeSeconds"]["value"], "93784",
        "1 d 2 h 3 min and 3.6 s, rounded"
    );
    Ok(())
}

#[test]
fn ingredients_with_resources_and_items_keep_quantity_quality_and_slot() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let magazine = blueprint::by_index(&catalog, 0)?;
    assert_eq!(
        magazine.presentation["ingredients"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
    assert_resource(
        &magazine,
        0,
        "b1000000-0000-4000-8000-000000000001",
        0.5,
        "0",
        Some("MAGAZINE"),
    )?;
    let (item, slot) = magazine.ingredient(1)?;
    assert_eq!(item["meaning"]["type"], "item");
    assert_eq!(
        item["meaning"]["itemGuid"]["value"],
        "e2000000-0000-4000-8000-000000000002"
    );
    assert_eq!(item["meaning"]["quantityUnits"]["value"], "5");
    assert_eq!(item["meaning"]["minQuality"]["value"], "500");
    let slot = slot.ok_or("nearest CORE slot")?;
    assert_eq!(magazine.debug_name(slot)?, "CORE");
    assert!(slot["meaning"]["displayName"]["value"].is_null());
    assert_resource(
        &magazine,
        2,
        "b2000000-0000-4000-8000-000000000002",
        0.25,
        "300",
        Some("CORE"),
    )?;
    assert_resource(
        &magazine,
        3,
        "b2000000-0000-4000-8000-000000000002",
        2.0,
        "100",
        Some("ASPECTS"),
    )?;
    let (_, slot) = magazine.ingredient(0)?;
    assert_eq!(
        magazine.game_key(&slot.ok_or("Magazine slot")?["meaning"]["displayName"])?,
        "@crafting_ui_slotname_magazine"
    );
    assert!(
        blueprint::by_index(&catalog, 1)?.ingredient(0)?.1.is_none(),
        "a cost outside any selection has no slot"
    );
    Ok(())
}

fn assert_resource(
    blueprint: &blueprint::Blueprint<'_>,
    index: usize,
    guid: &str,
    scu: f64,
    quality: &str,
    expected_slot: Option<&str>,
) -> TestResult {
    let (cost, slot) = blueprint.ingredient(index)?;
    let meaning = &cost["meaning"];
    assert_eq!(
        meaning["type"], "resource",
        "the cost retains its resource kind"
    );
    assert_eq!(
        meaning
            .pointer("/resourceGuid/value")
            .ok_or("authored resource GUID")?,
        guid,
        "the cost retains the authored resource link"
    );
    assert_eq!(
        meaning
            .pointer("/quantityScu/value")
            .and_then(Value::as_f64),
        Some(scu),
        "the cost retains its quantity in SCU"
    );
    assert_eq!(
        meaning
            .pointer("/minQuality/value")
            .ok_or("resource quality")?,
        quality,
        "the cost retains its minimum quality"
    );
    let name = slot.map(|slot| blueprint.debug_name(slot)).transpose()?;
    assert_eq!(
        name.and_then(Value::as_str),
        expected_slot,
        "the ingredient retains its nearest named slot"
    );
    Ok(())
}

#[test]
fn a_slot_is_inherited_through_unnamed_nested_selections() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let nested = blueprint::by_index(&catalog, 4)?;
    assert_eq!(nested.definition["className"]["value"], "BP_CRAFT_nested");
    assert_resource(
        &nested,
        0,
        "b1000000-0000-4000-8000-000000000001",
        3.0,
        "200",
        Some("OUTER"),
    )?;
    let (_, slot) = nested.ingredient(0)?;
    assert_eq!(
        nested.game_key(&slot.ok_or("Outer slot")?["meaning"]["displayName"])?,
        "@crafting_ui_slotname_outer"
    );
    Ok(())
}

#[test]
fn optional_costs_follow_the_mandatory_ones() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let nested = blueprint::by_index(&catalog, 4)?;
    assert_eq!(
        nested.presentation["ingredients"].as_array().map(Vec::len),
        Some(3),
        "one mandatory and two optional lines"
    );
    assert_resource(
        &nested,
        1,
        "b2000000-0000-4000-8000-000000000002",
        0.75,
        "400",
        None,
    )?;
    let (item, slot) = nested.ingredient(2)?;
    assert_eq!(
        item["meaning"]["itemGuid"]["value"],
        "e2000000-0000-4000-8000-000000000002"
    );
    assert_eq!(nested.debug_name(slot.ok_or("Extra slot")?)?, "EXTRA");
    Ok(())
}

#[test]
fn a_product_the_database_lacks_is_null() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let blueprint = blueprint::by_index(&catalog, 2)?;
    assert!(
        blueprint.presentation["producedItem"]["value"].is_null(),
        "dangling Reference"
    );
    let (item, _) = blueprint.ingredient(0)?;
    assert_eq!(
        item["meaning"]["itemGuid"]["value"], "e2000000-0000-4000-8000-000000000002",
        "the rest of the blueprint is read"
    );
    Ok(())
}

#[test]
fn a_filter_marks_both_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/metals/*");
    let (built, catalog) = catalog_crafting_build::build(Some(&filter))?;
    assert_eq!(commodity::class_names(&catalog)?, ["gold"], "filtered");
    assert!(class_names(&catalog)?.is_empty(), "filtered");
    for kind in ["commodities", "blueprints"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/metals/*"}),
            "{kind} is partial"
        );
    }
    assert_eq!(
        catalog["completeness"]["tags"],
        json!({"status": "complete"}),
        "the global Tag taxonomy retains its full selection scope"
    );
    assert!(
        built
            .summary()
            .kinds()
            .iter()
            .all(|entry| entry.complete == matches!(entry.kind, Kind::Tags | Kind::ShopInventories)),
        "the Tag taxonomy and physical shop inventories are globally selected"
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
