// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Crafting selection and completeness in catalog and library reads.

use p4kx_export::{Filter, Kind, read_crafting};
use p4kx_extraction::DataForge;
use serde_json::json;

use crate::catalog_crafting_support::{blueprint, build_from, text};
use crate::crafting_detail_fixture as fixture;

use super::TestResult;

#[test]
fn the_catalog_and_the_library_select_the_same_blueprints() -> TestResult {
    let filters = [None, Some(Filter::new("*/bp_craft_p*"))];
    for filter in filters {
        let (built, catalog) = build_from(&fixture::database(), filter.as_ref())?;
        let bytes = fixture::database().build();
        let forge = DataForge::open(&bytes)?;
        let crafting = read_crafting(&forge, filter.as_ref(), None)?;
        let from_catalog: Vec<&str> = blueprint::definitions(&catalog)?
            .iter()
            .filter_map(|entry| entry["className"]["value"].as_str())
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
    assert_eq!(blueprint::definitions(&catalog)?.len(), 1);
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
            .all(|entry| entry.complete == matches!(entry.kind, Kind::Tags | Kind::ShopInventories)),
        "the Tag taxonomy and physical shop inventories are globally selected"
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_crafting_kinds_and_the_skipped_blueprints() -> TestResult {
    let bytes = fixture::database().build();
    let forge = DataForge::open(&bytes)?;
    for record in forge.records() {
        assert_ne!(
            record.type_name()?,
            "BlueprintCategoryDatabaseRecord",
            "this original detailed fixture has no category-list source"
        );
    }
    let (built, _) = super::build()?;
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
            (Kind::BlueprintCategoryLists, 0, 0),
        ],
        "all original crafting counts and the appended source-empty listing kind"
    );
    assert!(built.summary().is_complete(), "no failure, no warning");
    Ok(())
}
