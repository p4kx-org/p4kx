// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A selection nested deeper than the reader follows fails its blueprint.

use std::error::Error;

use crate::catalog_crafting_support::{diagnostic_from, text};
use crate::crafting_detail::guid;
use crate::crafting_detail_fixture as fixture;
use p4kx_export::{Kind, read_crafting};
use p4kx_extraction::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_selection_nested_too_deep_fails_its_blueprint_instead_of_vanishing() -> TestResult {
    let bytes = fixture::nested_database(33).build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert!(
        crafting
            .blueprints
            .iter()
            .all(|blueprint| blueprint.guid != guid(fixture::TOO_DEEP)),
        "the blueprint is left out, not kept with a cut cost tree"
    );
    let [failure] = crafting.failures.as_slice() else {
        return Err(format!("one failure, found {:?}", crafting.failures).into());
    };
    assert_eq!(failure.guid, guid(fixture::TOO_DEEP));
    assert!(failure.reason.contains("deeper"), "{}", failure.reason);
    Ok(())
}

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
fn the_catalog_leaves_a_too_deep_blueprint_out_and_counts_the_failure() -> TestResult {
    let (built, catalog) = diagnostic_from(&fixture::nested_database(33), None)?;
    let listed = catalog["blueprints"].as_array().ok_or("blueprints")?;
    let deep = text(fixture::TOO_DEEP);
    assert!(
        listed.iter().all(|entry| entry["guid"] != deep.as_str()),
        "absent from blueprints"
    );
    let summary = built.summary();
    assert!(!summary.is_complete(), "a failure");
    let guids: Vec<String> = summary
        .failures()
        .iter()
        .map(|failure| failure.guid.to_string())
        .collect();
    assert_eq!(guids, [deep], "present in the failures of the summary");
    let blueprints = summary.kind(Kind::Blueprints).ok_or("blueprints")?;
    assert_eq!(blueprints.skipped, 1, "only the development leftover");
    Ok(())
}
