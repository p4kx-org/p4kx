// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The picks of a Loadout choose the alternative of each group of a generated Loadout; an index
//! beyond its group is an issue and the group keeps its first alternative.

use std::error::Error;

use p4kx_assembly::{LoadoutChanges, LoadoutPicks};
use p4kx_testkit::{TestLoadout, TestRecord, assortment_record};

use crate::loadout_assortment::{
    ASSORTMENT, attached, body, generated, kinds, pair, resolve_changed,
};

type TestResult = Result<(), Box<dyn Error>>;

/// Two groups: `Body` with two alternatives and `Egg` with three of the same Entity.
fn assortment() -> TestRecord {
    let egg = |class: &str| {
        TestLoadout::Manual(vec![p4kx_testkit::TestLoadoutEntry::class(
            "Egg_ItemPort",
            class,
        )])
    };
    assortment_record(
        ASSORTMENT,
        "beast",
        &[
            ("Body", vec![body("Body_Red"), body("Body_Blue")]),
            ("Egg", vec![egg("Egg"), egg("Body_Red"), egg("Body_Blue")]),
        ],
    )
}

fn picked(picks: LoadoutPicks) -> Result<p4kx_assembly::EntityNode, Box<dyn Error>> {
    let changes = LoadoutChanges::new().with_picks(picks);
    resolve_changed(
        generated(&[ASSORTMENT]),
        None,
        vec![assortment()],
        Some(&changes),
    )
}

#[test]
fn a_named_pick_chooses_the_alternative_of_its_group_only() -> TestResult {
    let tree = picked(LoadoutPicks::new().group("body", 1))?;

    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        attached(&tree),
        [
            pair("Body_ItemPort", "Body_Blue"),
            pair("Egg_ItemPort", "Egg")
        ],
        "Body takes its second alternative, Egg keeps its first"
    );
    Ok(())
}

#[test]
fn a_bare_index_chooses_it_in_every_group_that_has_it_without_an_issue() -> TestResult {
    let tree = picked(LoadoutPicks::new().every(2))?;

    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        attached(&tree),
        [
            pair("Body_ItemPort", "Body_Red"),
            pair("Egg_ItemPort", "Body_Blue")
        ],
        "Body has no third alternative and keeps its first, Egg takes its third"
    );
    Ok(())
}

#[test]
fn a_named_pick_wins_over_a_bare_index() -> TestResult {
    let tree = picked(LoadoutPicks::new().every(1).group("Egg", 2))?;

    assert_eq!(
        attached(&tree),
        [
            pair("Body_ItemPort", "Body_Blue"),
            pair("Egg_ItemPort", "Body_Blue")
        ]
    );
    Ok(())
}

#[test]
fn a_named_index_beyond_its_group_is_an_issue_and_the_group_keeps_its_first() -> TestResult {
    let tree = picked(LoadoutPicks::new().group("Body", 2))?;

    assert_eq!(kinds(&tree), ["assortment"]);
    let issue = tree.issues()[0].to_string();
    assert!(
        issue.contains("pick 2") && issue.contains("Body") && issue.contains("2 alternatives"),
        "{issue}"
    );
    assert_eq!(
        attached(&tree),
        [
            pair("Body_ItemPort", "Body_Red"),
            pair("Egg_ItemPort", "Egg")
        ]
    );
    Ok(())
}

#[test]
fn a_pick_of_the_first_alternative_is_no_issue() -> TestResult {
    let tree = picked(LoadoutPicks::new().group("Body", 0))?;

    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    Ok(())
}

#[test]
fn a_named_pick_of_a_group_the_loadout_lacks_is_an_issue_that_names_it() -> TestResult {
    let with = picked(LoadoutPicks::new().group("Bodyy", 2).group("egg", 1))?;
    let without = picked(LoadoutPicks::new())?;

    assert_eq!(
        kinds(&with),
        ["assortment"],
        "the known group egg is no issue"
    );
    let issue = with.issues()[0].to_string();
    assert!(issue.contains("Bodyy=2"), "{issue}");
    assert_eq!(
        attached(&with)[0],
        pair("Body_ItemPort", "Body_Red"),
        "the export is not changed by the typo"
    );
    assert_eq!(attached(&with).len(), attached(&without).len());
    Ok(())
}

#[test]
fn a_bare_index_beyond_every_group_is_no_issue() -> TestResult {
    let tree = picked(LoadoutPicks::new().every(9))?;

    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        attached(&tree),
        [
            pair("Body_ItemPort", "Body_Red"),
            pair("Egg_ItemPort", "Egg")
        ]
    );
    Ok(())
}

#[test]
fn a_huge_named_index_is_out_of_range_not_a_failure() -> TestResult {
    let tree = picked(LoadoutPicks::new().group("Body", usize::MAX))?;

    assert_eq!(kinds(&tree), ["assortment"]);
    assert_eq!(attached(&tree)[0], pair("Body_ItemPort", "Body_Red"));
    Ok(())
}

#[test]
fn the_pick_of_an_alternative_that_is_no_loadout_is_an_issue() -> TestResult {
    let record = assortment_record(
        ASSORTMENT,
        "unreadable",
        &[(
            "Body",
            vec![body("Body_Red"), TestLoadout::Xml(String::new())],
        )],
    );
    let changes = LoadoutChanges::new().with_picks(LoadoutPicks::new().group("Body", 1));
    let tree = resolve_changed(generated(&[ASSORTMENT]), None, vec![record], Some(&changes))?;

    assert_eq!(kinds(&tree), ["assortment"]);
    assert!(tree.issues()[0].to_string().contains("alternative 1"));
    Ok(())
}
