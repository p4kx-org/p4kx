// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Deep selection costs survive the complete read and preserve actual selection skips.

use std::error::Error;

use crate::catalog_crafting_support::{blueprint, build_from};
use crate::crafting_detail::guid;
use crate::crafting_detail_fixture as fixture;
use p4kx_export::{Kind, read_crafting};
use p4kx_extraction::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn thirty_two_nested_selections_are_kept() -> TestResult {
    let bytes = fixture::nested_database(32).build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert!(crafting.failures.is_empty(), "{:?}", crafting.failures);
    assert!(
        crafting
            .blueprints
            .iter()
            .any(|blueprint| blueprint.guid == guid(fixture::TOO_DEEP)),
        "the blueprint is kept"
    );
    Ok(())
}

#[test]
fn the_catalog_counts_the_entire_deep_blueprint_and_only_the_actual_leftover() -> TestResult {
    let (built, catalog) = build_from(&fixture::nested_database(33), None)?;
    let blueprint = blueprint::by_guid(&catalog, fixture::TOO_DEEP)?;
    let tiers = blueprint.tiers()?;
    let [tier] = tiers.as_slice() else {
        return Err("one original deep tier".into());
    };
    assert_eq!(tier["costs"].as_array().map(Vec::len), Some(34));
    let summary = built.summary();
    assert!(summary.is_complete());
    assert!(summary.failures().is_empty());
    let blueprints = summary.kind(Kind::Blueprints).ok_or("blueprints")?;
    assert_eq!(blueprints.skipped, 1, "only the development leftover");
    Ok(())
}
