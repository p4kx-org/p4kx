// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Requirement of an assortment and of its groups decides what a generated Loadout takes, and
//! what it cannot decide is a note of the Entity, never an issue.

use std::error::Error;

use p4kx_assembly::EntityNode;
use p4kx_testkit::{
    TestGroup, TestLoadout, TestLoadoutEntry, TestRecord, TestRequirement,
    assortment_record_requiring,
};

use crate::loadout_assortment::{ASSORTMENT, attached, generated, pair, resolve};

type TestResult = Result<(), Box<dyn Error>>;

const TAG: u128 = 0x500;

fn tagged() -> TestRequirement {
    TestRequirement::Tags {
        required: vec![TAG],
        banned: Vec::new(),
    }
}

fn never() -> TestRequirement {
    TestRequirement::Not(Box::new(TestRequirement::unrestricted()))
}

/// A Loadout putting the Entity `class` on the port `port`.
fn on(port: &str, class: &str) -> TestLoadout {
    TestLoadout::Manual(vec![TestLoadoutEntry::class(port, class)])
}

/// An assortment of two groups, `Body` and `Legs`, with the Requirements given.
fn requiring(
    assortment: Option<&TestRequirement>,
    body_group: Option<TestRequirement>,
    legs_group: Option<TestRequirement>,
) -> TestRecord {
    let group = |name: &str, port: &str, class: &str, requirement: Option<TestRequirement>| {
        let group = TestGroup::new(name, vec![on(port, class)]);
        match requirement {
            Some(requirement) => group.requiring(requirement),
            None => group,
        }
    };
    assortment_record_requiring(
        ASSORTMENT,
        "required",
        assortment,
        &[
            group("Body", "Body_ItemPort", "Body_Red", body_group),
            group("Legs", "Legs_ItemPort", "Body_Blue", legs_group),
        ],
    )
}

/// What both groups of [`requiring`] attach.
fn both() -> Vec<(String, String)> {
    vec![
        pair("Body_ItemPort", "Body_Red"),
        pair("Legs_ItemPort", "Body_Blue"),
    ]
}

/// What only the `Legs` group attaches.
fn legs_only() -> Vec<(String, String)> {
    vec![pair("Legs_ItemPort", "Body_Blue")]
}

fn resolved(record: TestRecord) -> Result<EntityNode, Box<dyn Error>> {
    resolve(generated(&[ASSORTMENT]), None, vec![record])
}

fn notes(node: &EntityNode) -> Vec<String> {
    node.notes().iter().map(ToString::to_string).collect()
}

#[test]
fn a_requirement_that_restricts_nothing_applies_without_a_note() -> TestResult {
    let record = requiring(
        Some(&TestRequirement::unrestricted()),
        Some(TestRequirement::unrestricted()),
        Some(TestRequirement::AnyOf(vec![
            tagged(),
            TestRequirement::unrestricted(),
        ])),
    );
    let tree = resolved(record)?;

    assert!(tree.issues().is_empty() && tree.notes().is_empty());
    assert_eq!(attached(&tree), both());
    Ok(())
}

#[test]
fn a_group_that_depends_on_tags_still_applies_and_is_noted_not_an_issue() -> TestResult {
    let tree = resolved(requiring(None, Some(tagged()), None))?;

    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    let notes = notes(&tree);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("the group Body"), "{notes:?}");
    assert_eq!(tree.notes()[0].kind(), "assortment_requirement");
    assert!(tree.notes()[0].is_note());
    assert_eq!(attached(&tree), both());
    Ok(())
}

#[test]
fn an_assortment_that_depends_on_tags_still_applies_and_is_noted() -> TestResult {
    let tree = resolved(requiring(Some(&tagged()), None, None))?;

    assert_eq!(tree.issues(), []);
    assert!(notes(&tree)[0].contains("the assortment"));
    assert_eq!(attached(&tree), both());
    Ok(())
}

#[test]
fn several_undecided_groups_and_a_second_reference_make_one_note() -> TestResult {
    let record = requiring(Some(&tagged()), Some(tagged()), Some(tagged()));
    let tree = resolve(generated(&[ASSORTMENT, ASSORTMENT]), None, vec![record])?;

    let notes = notes(&tree);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].contains("the assortment and the groups Body, Legs"),
        "{notes:?}"
    );
    Ok(())
}

#[test]
fn a_group_whose_requirement_fails_gives_nothing() -> TestResult {
    let record = assortment_record_requiring(
        ASSORTMENT,
        "never",
        None,
        &[
            TestGroup::new("Body", vec![on("Body_ItemPort", "Body_Red")]).requiring(never()),
            TestGroup::new("Legs", vec![on("Legs_ItemPort", "Body_Blue")]),
        ],
    );
    let tree = resolved(record)?;

    assert!(tree.issues().is_empty() && tree.notes().is_empty());
    assert_eq!(attached(&tree), legs_only());
    Ok(())
}

#[test]
fn an_assortment_whose_requirement_fails_gives_nothing() -> TestResult {
    let tree = resolved(requiring(Some(&never()), None, None))?;

    assert!(tree.issues().is_empty() && tree.notes().is_empty());
    assert_eq!(attached(&tree), Vec::<(String, String)>::new());
    Ok(())
}

#[test]
fn not_and_or_follow_their_parts() -> TestResult {
    let restricts_nothing =
        || TestRequirement::AnyOf(vec![tagged(), TestRequirement::unrestricted()]);
    let cases = [
        (TestRequirement::Not(Box::new(tagged())), true, both()),
        (TestRequirement::AnyOf(Vec::new()), true, both()),
        (TestRequirement::AnyOf(vec![never()]), false, legs_only()),
        (restricts_nothing(), false, both()),
        (
            TestRequirement::Not(Box::new(restricts_nothing())),
            false,
            legs_only(),
        ),
    ];
    for (requirement, noted, expected) in cases {
        let tree = resolved(requiring(None, Some(requirement.clone()), None))?;
        assert_eq!(!tree.notes().is_empty(), noted, "{requirement:?}");
        assert_eq!(attached(&tree), expected, "{requirement:?}");
    }
    Ok(())
}
