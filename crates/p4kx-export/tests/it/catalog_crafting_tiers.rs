// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tiers, selections, quality, pools, reward sources and dismantle returns in the Basetool
//! catalog, built from a synthetic Archive with multi-tier blueprints (no blueprint of the
//! real Archive has more than one tier).

#[path = "catalog_crafting_quality.rs"]
mod quality;
#[path = "catalog_crafting_tier_selection.rs"]
mod selection;

use crate::catalog_crafting_support;
use crate::crafting_detail_fixture;

use std::error::Error;

use catalog_crafting_support::{blueprint, build_from, diagnostic_from, text};
use crafting_detail_fixture as fixture;
use p4kx_export::{BuiltCatalog, CraftingWarning};
use serde::Deserialize;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_from(&fixture::database(), None)
}

#[test]
fn a_blueprint_lists_every_tier_next_to_the_first_tier_fields() -> TestResult {
    let (_, catalog) = build()?;
    let blueprint = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    let tiers = blueprint.tiers()?;
    let [first, second] = tiers.as_slice() else {
        return Err("both original tiers".into());
    };
    assert_eq!(first["craftTimeSeconds"]["value"], "90");
    assert_eq!(second["craftTimeSeconds"]["value"], "120");
    assert_eq!(blueprint.presentation["firstTier"], first["source"]);
    assert_eq!(
        blueprint.presentation["ingredients"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    for (index, expected) in [(0, Some("FRAME")), (1, Some("CORE")), (2, None)] {
        let (_, slot) = blueprint.ingredient(index)?;
        let name = slot.map(|slot| blueprint.debug_name(slot)).transpose()?;
        assert_eq!(
            name.and_then(Value::as_str),
            expected,
            "the first tier is flattened in order using the nearest named slot"
        );
    }
    let bare = blueprint::by_guid(&catalog, fixture::BARE)?;
    assert!(bare.tiers()?.is_empty(), "no tier");
    assert!(bare.presentation["firstTier"].is_null());
    Ok(())
}

#[test]
fn a_selection_keeps_its_count_and_all_its_options() -> TestResult {
    let (_, catalog) = build()?;
    let blueprint = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    let tiers = blueprint.tiers()?;
    let first = tiers.first().copied().ok_or("first Blueprint tier")?;
    let outer = blueprint::cost(first, &first["mandatory"])?;
    let meaning = &outer["meaning"];
    assert_eq!(meaning["type"], "select");
    assert_eq!(meaning["count"]["value"], "2");
    assert_eq!(blueprint.debug_name(outer)?, "ASPECTS");
    assert_eq!(
        blueprint.game_key(&meaning["displayName"])?,
        "@crafting_slot"
    );
    assert_eq!(meaning["options"].as_array().map(Vec::len), Some(2));

    let chosen = blueprint::by_guid(&catalog, fixture::CHOOSE_ONE)?;
    let tiers = chosen.tiers()?;
    let tier = tiers.first().copied().ok_or("choose-one tier")?;
    let pick = blueprint::cost(tier, &tier["mandatory"])?;
    let options = &pick["meaning"]["options"];
    assert_eq!(pick["meaning"]["count"]["value"], "1");
    assert_eq!(options.as_array().map(Vec::len), Some(3));
    assert_eq!(
        blueprint::cost(tier, &options[2])?["meaning"]["type"],
        "item"
    );

    let optional = blueprint::cost(first, &first["optional"][0])?;
    assert_eq!(optional["meaning"]["type"], "item");
    assert_eq!(
        optional["meaning"]["itemGuid"]["value"],
        text(fixture::PRODUCT_B)
    );
    assert_eq!(optional["meaning"]["quantityUnits"]["value"], "3");
    Ok(())
}

#[test]
fn a_cost_line_keeps_its_quantity_multiplier_and_leaves_the_quantity_alone() -> TestResult {
    let (_, catalog) = build()?;
    let blueprint = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    let (line, _) = blueprint.ingredient(0)?;
    assert_eq!(line["meaning"]["type"], "resource");
    assert_eq!(line["meaning"]["quantityScu"]["value"], 0.5);
    assert_eq!(line["context"]["quantityMultiplier"]["value"], 1.15);
    assert_eq!(line["meaning"]["minQuality"]["value"], "300");
    let (included, _) = blueprint.ingredient(1)?;
    assert!(included["context"]["quantityMultiplier"].is_null());
    let option = blueprint.node(&included["context"]["compositionInclusion"])?;
    let authored = &option["value"];
    assert_eq!(
        authored["value"]["type"], "string",
        "the source fixture declares a String"
    );
    assert_eq!(authored["value"]["value"]["type"], "gameText");
    assert_eq!(authored["value"]["value"]["sourceText"], "Include");
    assert_eq!(authored["containsEncryptedSource"], true);
    assert_eq!(authored["sourceSetComplete"], true);
    Ok(())
}

#[test]
fn quality_modifiers_name_their_property_and_keep_their_ranges() -> TestResult {
    let database = fixture::database();
    let property_source = crate::catalog_crafting_property::original(
        &database,
        fixture::PROPERTY,
        "@StatName_GPP_Armor",
    )?;
    let (built, catalog) = build_from(&database, None)?;
    let blueprint = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    let tiers = blueprint.tiers()?;
    let second = tiers.get(1).copied().ok_or("second original tier")?;
    let selection = blueprint::cost(second, &second["mandatory"])?;
    let line = blueprint::cost(second, &selection["meaning"]["options"][0])?;
    let modifiers = line["context"]["modifiers"].as_array().ok_or("modifiers")?;
    let [modifier] = modifiers.as_slice() else {
        return Err("one modifier".into());
    };
    assert_eq!(modifier["propertyGuid"]["value"], text(fixture::PROPERTY));
    let ranges = modifier["ranges"].as_array().ok_or("modifier ranges")?;
    let [linear, additive] = ranges.as_slice() else {
        return Err("two ranges".into());
    };
    assert_range(linear, "multiplier", "1000", 0.85, 1.15)?;
    assert_range(additive, "additive", "249", -2.0, -1.0)?;
    crate::catalog_crafting_property::assert_armour(&built, &property_source)?;
    Ok(())
}

fn assert_range(range: &Value, kind: &str, end: &str, at_start: f32, at_end: f32) -> TestResult {
    assert_eq!(range["kind"], kind, "the original modifier kind survives");
    assert_eq!(
        range
            .pointer("/startQuality/value")
            .ok_or("minimum modifier quality")?,
        "0",
        "the original minimum quality survives"
    );
    assert_eq!(
        range
            .pointer("/endQuality/value")
            .ok_or("maximum modifier quality")?,
        end,
        "the original maximum quality survives"
    );
    for (name, expected) in [("atStart", at_start), ("atEnd", at_end)] {
        let cell = &range[name];
        let actual = f32::deserialize(&cell["value"])?;
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "exact finite f32 round-trip"
        );
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "the modifier endpoint comes from encrypted source data"
        );
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the modifier endpoint retains complete source provenance"
        );
    }
    Ok(())
}

#[test]
fn quality_distributions_keep_their_location_overrides() -> TestResult {
    let (_, catalog) = build()?;
    quality::assert_location_overrides(&catalog)
}

#[test]
fn a_pool_lists_its_entries_and_the_places_that_reward_it() -> TestResult {
    let (_, catalog) = build()?;
    let pools = catalog["blueprintPools"].as_array().ok_or("pools")?;
    let pool = pools
        .iter()
        .find(|pool| pool["guid"] == text(fixture::POOL).as_str())
        .ok_or("the pool")?;
    assert_eq!(pool["totalWeight"], 4.0);
    assert_eq!(
        pool["entries"],
        json!([
            {"blueprintGuid": text(fixture::TWO_TIERS), "weight": 1.0},
            {"blueprintGuid": text(fixture::CHOOSE_ONE), "weight": 3.0},
        ])
    );
    assert_eq!(
        pool["rewardSources"],
        json!([{
            "recordGuid": text(fixture::CONTRACTS),
            "recordPath": "libs/foundry/records/contracts/contractgenerator/haul",
            "contractId": text(fixture::CONTRACT_ID),
            "contractName": "Haul_Intro",
            "chance": 0.5,
        }]),
        "the chance of the source; a blueprint's share is weight over totalWeight"
    );
    let bad = pools
        .iter()
        .find(|pool| pool["guid"] == text(fixture::BAD_POOL).as_str())
        .ok_or("the pool with a negative weight")?;
    assert_eq!(bad["totalWeight"], 2.0, "only positive weights count");
    Ok(())
}

#[test]
fn a_blueprint_links_to_the_dismantle_process_unless_its_item_is_excluded() -> TestResult {
    let (_, catalog) = build()?;
    assert_eq!(
        blueprint::by_guid(&catalog, fixture::TWO_TIERS)?.presentation["dismantle"]["value"]["processGuid"],
        text(fixture::DISMANTLE)
    );
    assert_eq!(
        blueprint::by_guid(&catalog, fixture::BARE)?.presentation["dismantle"]["state"],
        "absent",
        "the item is on the dismantle blacklist"
    );
    let blueprint = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    assert_eq!(
        blueprint.presentation["dismantle"]["value"]["tier"],
        blueprint.presentation["firstTier"]
    );
    assert_eq!(
        catalog["dismantleProcesses"],
        json!([{
            "guid": text(fixture::DISMANTLE),
            "className": "GlobalGenericDismantle",
            "recordPath": "libs/foundry/records/crafting/blueprints/dismantle/globalgenericdismantle",
            "efficiency": 0.5,
            "timeSeconds": 15,
        }])
    );
    assert_eq!(
        catalog["craftingGlobal"],
        json!({
            "dismantleExcludedResourceGuids": [text(fixture::GOLD)],
            "dismantleExcludedItemGuids": [text(fixture::BLACKLISTED)],
        })
    );
    Ok(())
}

#[test]
fn a_default_blueprint_is_marked_available() -> TestResult {
    let (_, catalog) = build()?;
    let mut flags = Vec::new();
    for index in 0..blueprint::definitions(&catalog)?.len() {
        let blueprint = blueprint::by_index(&catalog, index)?;
        let class = blueprint.definition["className"]["value"]
            .as_str()
            .ok_or("original class")?;
        let available = blueprint.presentation["defaultAvailable"]["value"]
            .as_bool()
            .ok_or("default flag")?;
        flags.push((class.to_owned(), available));
    }
    assert_eq!(
        flags,
        [
            ("BP_CRAFT_two_tiers".to_owned(), false),
            ("BP_CRAFT_pick".to_owned(), true),
            ("BP_CRAFT_bare".to_owned(), false),
        ],
        "only the blueprint of defaultBlueprintSelection"
    );
    Ok(())
}

#[test]
fn several_dismantle_processes_leave_no_link_and_the_summary_incomplete() -> TestResult {
    let (built, catalog) = diagnostic_from(&fixture::ambiguous_database(), None)?;
    for index in 0..blueprint::definitions(&catalog)?.len() {
        let blueprint = blueprint::by_index(&catalog, index)?;
        assert_eq!(
            blueprint.presentation["dismantle"]["state"], "absent",
            "{}",
            blueprint.definition["className"]
        );
    }
    assert_eq!(
        catalog["dismantleProcesses"].as_array().map(Vec::len),
        Some(2),
        "both processes are listed"
    );
    assert_eq!(
        built.summary().warnings(),
        [CraftingWarning::AmbiguousDismantle(2)]
    );
    assert!(!built.summary().is_complete());
    Ok(())
}
