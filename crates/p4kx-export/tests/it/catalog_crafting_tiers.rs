// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tiers, selections, quality, pools, reward sources and dismantle returns in the Basetool
//! catalog, built from a synthetic Archive with multi-tier blueprints (no blueprint of the
//! real Archive has more than one tier).

use crate::catalog_crafting_support;
use crate::crafting_detail_fixture;

use std::error::Error;

use catalog_crafting_support::{blueprint, build_from, diagnostic_from, text};
use crafting_detail_fixture as fixture;
use p4kx_export::{BuiltCatalog, CraftingWarning, Filter, Kind, read_crafting};
use p4kx_extraction::DataForge;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_from(&fixture::database(), None)
}

#[test]
fn a_blueprint_lists_every_tier_next_to_the_first_tier_fields() -> TestResult {
    let (_, catalog) = build()?;
    let blueprint = blueprint(&catalog, fixture::TWO_TIERS)?;
    let tiers = blueprint["tiers"].as_array().ok_or("tiers")?;
    assert_eq!(tiers.len(), 2, "both tiers");
    assert_eq!(
        (
            &tiers[0]["craftTimeSeconds"],
            &tiers[1]["craftTimeSeconds"],
            &blueprint["craftTimeSeconds"]
        ),
        (&json!(90), &json!(120), &json!(90)),
        "the first tier is the one craftTimeSeconds describes"
    );
    assert_eq!(
        blueprint["ingredients"]
            .as_array()
            .map(|list| list.iter().map(|line| line["slot"].clone()).collect()),
        Some(vec![json!("FRAME"), json!("CORE"), Value::Null]),
        "the ingredients are the first tier flattened; the optional item sits in no slot"
    );
    let bare = self::blueprint(&catalog, fixture::BARE)?;
    assert_eq!(bare["tiers"], json!([]), "no tier");
    assert_eq!(bare["craftTimeSeconds"], Value::Null);
    Ok(())
}

#[test]
fn a_selection_keeps_its_count_and_all_its_options() -> TestResult {
    let (_, catalog) = build()?;
    let outer = &blueprint(&catalog, fixture::TWO_TIERS)?["tiers"][0]["mandatory"];
    assert_eq!(
        (
            &outer["kind"],
            &outer["count"],
            &outer["slot"],
            &outer["slotKey"]
        ),
        (
            &json!("select"),
            &json!(2),
            &json!("ASPECTS"),
            &json!("@crafting_slot")
        ),
    );
    assert_eq!(outer["options"].as_array().map(Vec::len), Some(2));
    let pick = &blueprint(&catalog, fixture::CHOOSE_ONE)?["tiers"][0]["mandatory"];
    assert_eq!(
        (&pick["count"], pick["options"].as_array().map(Vec::len)),
        (&json!(1), Some(3)),
        "choose 1 of 3: fewer than offered"
    );
    assert_eq!(
        pick["options"][2]["kind"], "item",
        "an item option inside the selection"
    );
    let optional = &blueprint(&catalog, fixture::TWO_TIERS)?["tiers"][0]["optional"];
    assert_eq!(
        (
            &optional[0]["kind"],
            &optional[0]["itemGuid"],
            &optional[0]["quantityUnits"]
        ),
        (&json!("item"), &json!(text(fixture::PRODUCT_B)), &json!(3)),
        "an optional cost"
    );
    Ok(())
}

#[test]
fn a_cost_line_keeps_its_quantity_multiplier_and_leaves_the_quantity_alone() -> TestResult {
    let (_, catalog) = build()?;
    let blueprint = blueprint(&catalog, fixture::TWO_TIERS)?;
    let line = &blueprint["tiers"][0]["mandatory"]["options"][0]["options"][0];
    assert_eq!(
        (
            &line["kind"],
            &line["quantityScu"],
            &line["quantityMultiplier"],
            &line["minQuality"]
        ),
        (&json!("resource"), &json!(0.5), &json!(1.15), &json!(300))
    );
    let included = &blueprint["tiers"][0]["mandatory"]["options"][1]["options"][0];
    assert_eq!(
        (
            &included["quantityMultiplier"],
            &included["compositionInclusion"]
        ),
        (&Value::Null, &json!("Include"))
    );
    assert_eq!(
        blueprint["ingredients"][0]["quantityScu"], 0.5,
        "the ingredient keeps the stored quantity"
    );
    Ok(())
}

#[test]
fn quality_modifiers_name_their_property_and_keep_their_ranges() -> TestResult {
    let (_, catalog) = build()?;
    let line = &blueprint(&catalog, fixture::TWO_TIERS)?["tiers"][1]["mandatory"]["options"][0];
    assert_eq!(
        line["modifiers"],
        json!([{
            "propertyGuid": text(fixture::PROPERTY),
            "ranges": [
                {"kind": "multiplier", "startQuality": 0, "endQuality": 1000,
                 "atStart": 0.85, "atEnd": 1.15},
                {"kind": "additive", "startQuality": 0, "endQuality": 249,
                 "atStart": -2.0, "atEnd": -1.0},
            ],
        }])
    );
    assert_eq!(
        catalog["gameplayProperties"],
        json!([{
            "guid": text(fixture::PROPERTY),
            "className": "GPP_Armor_DamageMitigation",
            "recordPath": "libs/foundry/records/crafting/craftedproperties/gpp_armor_damagemitigation",
            "name": {"key": "@StatName_GPP_Armor", "en": "Armour", "de": null},
        }]),
        "the property the modifier points at"
    );
    Ok(())
}

#[test]
fn quality_distributions_keep_their_location_overrides() -> TestResult {
    let (_, catalog) = build()?;
    let list = catalog["qualityDistributions"].as_array().ok_or("list")?;
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["guid"], text(fixture::DEFAULT_CURVE).as_str());
    assert_eq!(
        list[0]["distribution"],
        json!({"kind": "CraftingQualityDistributionNormal",
               "min": 1, "max": 1000, "mean": 269.0, "stddev": 254.0})
    );
    assert_eq!(list[0]["overrides"], json!([]));
    assert_eq!(list[1]["distribution"], Value::Null);
    assert_eq!(
        list[1]["overrides"][0]["locationGuid"],
        text(fixture::PYRO).as_str()
    );
    assert_eq!(
        list[1]["overrides"][0]["distribution"]["mean"], 295.0,
        "the override curve"
    );
    assert_eq!(
        list[1]["overrides"][0]["distribution"]["stddev"], 1.0,
        "the finite override standard deviation"
    );
    Ok(())
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
        blueprint(&catalog, fixture::TWO_TIERS)?["dismantle"],
        json!({"processGuid": text(fixture::DISMANTLE), "tierIndex": 0})
    );
    assert_eq!(
        blueprint(&catalog, fixture::BARE)?["dismantle"],
        Value::Null,
        "the item is on the dismantle blacklist"
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
    let flags: Vec<(String, bool)> = catalog["blueprints"]
        .as_array()
        .ok_or("blueprints")?
        .iter()
        .map(|entry| {
            (
                entry["className"].as_str().unwrap_or_default().to_owned(),
                entry["defaultAvailable"].as_bool().unwrap_or_default(),
            )
        })
        .collect();
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
    for entry in catalog["blueprints"].as_array().ok_or("blueprints")? {
        assert_eq!(entry["dismantle"], Value::Null, "{}", entry["className"]);
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

#[test]
fn the_catalog_and_the_library_select_the_same_blueprints() -> TestResult {
    let filters = [None, Some(Filter::new("*/bp_craft_p*"))];
    for filter in filters {
        let (built, catalog) = build_from(&fixture::database(), filter.as_ref())?;
        let bytes = fixture::database().build();
        let forge = DataForge::open(&bytes)?;
        let crafting = read_crafting(&forge, filter.as_ref(), None)?;
        let from_catalog: Vec<&str> = catalog["blueprints"]
            .as_array()
            .ok_or("blueprints")?
            .iter()
            .filter_map(|entry| entry["className"].as_str())
            .collect();
        let mut from_library: Vec<&str> =
            crafting.blueprints.iter().map(|b| b.class_name).collect();
        from_library.sort_unstable();
        let mut sorted = from_catalog.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, from_library, "filter {filter:?}");
        let kind = built.summary().kind(Kind::Blueprints).ok_or("summary")?;
        assert_eq!(
            (kind.count, kind.skipped),
            (crafting.blueprints.len(), crafting.skipped_blueprints)
        );
    }
    Ok(())
}

#[test]
fn a_filter_marks_the_crafting_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/bp_craft_pick");
    let (built, catalog) = build_from(&fixture::database(), Some(&filter))?;
    for kind in [
        "blueprints",
        "gameplayProperties",
        "qualityDistributions",
        "qualityQuantizations",
        "blueprintPools",
        "dismantleProcesses",
    ] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/bp_craft_pick"}),
            "{kind}"
        );
    }
    assert_eq!(catalog["blueprints"].as_array().map(Vec::len), Some(1));
    assert_eq!(catalog["blueprintPools"], json!([]), "no pool matches");
    assert_eq!(catalog["dismantleProcesses"], json!([]), "filtered by path");
    assert_eq!(
        catalog["craftingGlobal"]["dismantleExcludedResourceGuids"],
        json!([text(fixture::GOLD)]),
        "the global parameters are not a kind and stay"
    );
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
            .all(|entry| entry.complete == (entry.kind == Kind::Tags)),
        "every scoped kind is partial; only the global Tag taxonomy stays complete"
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_crafting_kinds_and_the_skipped_blueprints() -> TestResult {
    let (built, _) = build()?;
    let counts: Vec<(Kind, usize, usize)> = built
        .summary()
        .kinds()
        .iter()
        .filter(|kind| kind.kind.name().starts_with(['b', 'g', 'q', 'd']))
        .map(|kind| (kind.kind, kind.count, kind.skipped))
        .collect();
    assert_eq!(
        counts,
        [
            (Kind::Blueprints, 3, 1),
            (Kind::BlueprintCategories, 0, 0),
            (Kind::GameplayProperties, 1, 0),
            (Kind::QualityDistributions, 2, 0),
            (Kind::QualityQuantizations, 0, 0),
            (Kind::BlueprintPools, 3, 1),
            (Kind::DismantleProcesses, 1, 0),
        ]
    );
    assert!(built.summary().is_complete(), "no failure, no warning");
    Ok(())
}
