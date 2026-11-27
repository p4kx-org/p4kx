// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityNode, LoadoutError};
use p4kx_dataforge::Value;
use p4kx_testkit::{TestInstance, TestRequirement, TestValue};

use crate::complete_requirement_fixture::{self as fixture, Site, TestResult};
use crate::complete_requirement_source as source;
use crate::loadout_assortment::{attached, pair};

fn check(node: &EntityNode, ports: &[(String, String)], uncertain: bool) {
    assert!(
        node.issues().is_empty(),
        "genuine source has no failure issue: {:?}",
        node.issues()
    );
    assert_eq!(
        attached(node),
        ports,
        "exact ordered selection from actual original Requirement"
    );
    let notes: Vec<_> = node.notes().iter().map(LoadoutError::kind).collect();
    assert_eq!(
        notes,
        if uncertain {
            vec!["assortment_requirement"]
        } else {
            Vec::new()
        },
        "only genuinely undecidable input produces the ordinary note"
    );
}

fn both() -> Vec<(String, String)> {
    vec![
        pair("Body_ItemPort", "Body_Red"),
        pair("Legs_ItemPort", "Body_Blue"),
    ]
}

fn selected_chain(site: Site, depth: usize, ports: &[(String, String)]) -> TestResult {
    let requirement = fixture::nested(depth, TestRequirement::unrestricted());
    let bytes = fixture::bytes(site, Some(requirement));
    fixture::run(
        &bytes,
        |tree| {
            source::chain(
                source::requirement(tree, site)?.ok_or("original Requirement")?,
                depth,
                false,
            )
        },
        |node| check(node, ports, false),
    )
}

#[test]
fn requirement_complete_seventeen_nots_exclude_the_original_assortment() -> TestResult {
    selected_chain(Site::Assortment, 17, &[])
}

#[test]
fn requirement_complete_seventeen_nots_exclude_only_the_original_group() -> TestResult {
    selected_chain(Site::Group, 17, &[pair("Legs_ItemPort", "Body_Blue")])
}

#[test]
fn requirement_complete_eighteen_nots_include_without_a_false_note() -> TestResult {
    selected_chain(Site::Assortment, 18, &both())?;
    selected_chain(Site::Group, 18, &both())
}

#[test]
fn requirement_complete_or_of_deep_and_shallow_unmet_parts_excludes() -> TestResult {
    let requirement = TestRequirement::AnyOf(vec![
        fixture::nested(17, TestRequirement::unrestricted()),
        fixture::nested(1, TestRequirement::unrestricted()),
    ]);
    let bytes = fixture::bytes(Site::Assortment, Some(requirement));
    fixture::run(
        &bytes,
        |tree| {
            let parts =
                source::parts(source::requirement(tree, Site::Assortment)?.ok_or("original OR")?)?;
            assert_eq!(parts.len(), 2, "both actual OR operands");
            source::chain(parts.first().ok_or("original first operand")?, 17, false)?;
            source::chain(parts.get(1).ok_or("original second operand")?, 1, false)
        },
        |node| check(node, &[], false),
    )
}

#[test]
fn requirement_complete_deep_genuine_tags_remain_undecidable() -> TestResult {
    for site in [Site::Assortment, Site::Group] {
        let bytes = fixture::bytes(site, Some(fixture::nested(18, fixture::tagged())));
        fixture::run(
            &bytes,
            |tree| {
                source::chain(
                    source::requirement(tree, site)?.ok_or("original tagged Requirement")?,
                    18,
                    true,
                )
            },
            |node| check(node, &both(), true),
        )?;
    }
    Ok(())
}

#[test]
fn requirement_complete_absent_and_null_root_stay_unrestricted() -> TestResult {
    let bytes = fixture::bytes(Site::Assortment, None);
    fixture::run(
        &bytes,
        |tree| {
            assert!(
                matches!(
                    source::requirement(tree, Site::Assortment)?,
                    Some(Value::Null)
                ),
                "actual declared root null pointer"
            );
            Ok(())
        },
        |node| check(node, &both(), false),
    )?;
    fixture::run(
        &fixture::missing()?,
        |tree| {
            assert!(
                source::requirement(tree, Site::Assortment)?.is_none(),
                "actual missing original field"
            );
            assert!(
                matches!(tree.field("requirement"), Some(Value::Null)),
                "renamed source declaration really exists"
            );
            Ok(())
        },
        |node| check(node, &both(), false),
    )
}

#[test]
fn requirement_complete_unknown_type_empty_or_and_nested_null_remain_unknown() -> TestResult {
    for name in [
        "SLoadoutRequirementBase",
        "SLoadoutRequirementOR",
        "SLoadoutRequirementNOT",
    ] {
        let bytes = fixture::raw(TestValue::Struct(TestInstance::new(name)));
        fixture::run(
            &bytes,
            |tree| {
                let value =
                    source::requirement(tree, Site::Assortment)?.ok_or("actual Requirement")?;
                let part = source::structure(value)?;
                assert_eq!(
                    part.type_name(),
                    name,
                    "actual original unknown or empty form"
                );
                match name {
                    "SLoadoutRequirementOR" => {
                        assert!(source::parts(value)?.is_empty(), "actual empty OR");
                    }
                    "SLoadoutRequirementNOT" => assert!(
                        matches!(part.field("RequirementToInvert"), Some(Value::Null)),
                        "actual nested null is not an unrestricted root"
                    ),
                    _ => assert!(
                        part.fields().is_empty(),
                        "actual fieldless unknown base type"
                    ),
                }
                Ok(())
            },
            |node| check(node, &both(), true),
        )?;
    }
    Ok(())
}

#[test]
fn requirement_complete_decisive_or_preserves_order_and_no_unknown_note() -> TestResult {
    for first_met in [true, false] {
        let parts = if first_met {
            vec![TestRequirement::unrestricted(), fixture::tagged()]
        } else {
            vec![fixture::tagged(), TestRequirement::unrestricted()]
        };
        let bytes = fixture::bytes(Site::Assortment, Some(TestRequirement::AnyOf(parts)));
        fixture::run(
            &bytes,
            |tree| {
                let parts = source::parts(
                    source::requirement(tree, Site::Assortment)?.ok_or("original decisive OR")?,
                )?;
                assert_eq!(parts.len(), 2, "both original operands before evaluation");
                source::tags(parts.first().ok_or("original first operand")?, !first_met)?;
                source::tags(parts.get(1).ok_or("original second operand")?, first_met)
            },
            |node| check(node, &both(), false),
        )?;
    }
    Ok(())
}
