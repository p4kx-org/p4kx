// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::accounting::{self, Counts, DeclarationState, Projection, Review};
use super::inventory::{BuildIdentity, FieldKey, Inventory};
use super::schema::{Declaration, Schema};
use super::source::Observations;

const OWNER: &str = "FixtureQuantumDrive";
const BOOST: &str = "boost";
const HEAT: &str = "heat";

fn build_claim() -> Value {
    json!({
        "channel": "TECH-PREVIEW",
        "branch": "sc-alpha-4.10.0",
        "version": "4.10.200.1",
        "changeNumber": "123",
        "buildId": "fixture",
    })
}

fn expected_absence() -> Value {
    json!({
        "source": {"owner": OWNER, "field": BOOST, "arrayElement": false},
        "build": build_claim(),
        "reason": "Reviewed declaration is absent from the fixture manifest",
    })
}

fn inventory_value() -> Value {
    let mapping = |field| {
        json!({
            "source": {"owner": OWNER, "field": field, "arrayElement": false},
            "declaredTarget": null,
            "disposition": {"kind": "metadata", "reason": "Fixture field"},
            "previouslyEmpty": false,
        })
    };
    json!({
        "schemaVersion": 1,
        "roots": [OWNER],
        "fields": [mapping(BOOST), mapping(HEAT)],
    })
}

fn inventory_with(absences: Vec<Value>) -> super::Result<Inventory> {
    let mut input = inventory_value();
    input
        .as_object_mut()
        .ok_or("fixture inventory object")?
        .insert("expectedAbsences".to_owned(), Value::Array(absences));
    Ok(serde_json::from_value(input)?)
}

#[derive(Clone, Copy)]
enum SourceShape {
    OwnerMissing,
    BoostMissing,
    BoostDeclared,
}

fn survey(
    inventory: &Inventory,
    build: Option<&BuildIdentity>,
    shape: SourceShape,
) -> super::Result<BTreeMap<FieldKey, Counts>> {
    let declarations = matches!(shape, SourceShape::BoostDeclared)
        .then(|| {
            (
                FieldKey::new(OWNER, BOOST),
                Declaration {
                    target: None,
                    array: false,
                },
            )
        })
        .into_iter()
        .collect();
    let owners = (!matches!(shape, SourceShape::OwnerMissing))
        .then(|| (OWNER.to_owned(), BTreeMap::new()))
        .into_iter()
        .collect();
    accounting::count_fields(
        &Schema {
            declarations,
            owners,
        },
        &Observations::new(),
        inventory,
        &Projection {
            groups: BTreeMap::new(),
            rejected: BTreeSet::new(),
            exclusions: BTreeMap::new(),
        },
        build,
        &|| false,
    )
}

#[test]
fn an_expected_absence_applies_only_to_the_reviewed_field_in_its_exact_manifest()
-> super::Result<()> {
    let temp = tempfile::tempdir()?;
    let archive = super::real_archive::build_fixture(&temp.path().join("TECH-PREVIEW"), "123")?;
    let build = BuildIdentity::of_archive(&archive).ok_or("exact fixture manifest")?;
    let inventory = inventory_with(vec![expected_absence()])?;
    let counts = survey(&inventory, Some(&build), SourceShape::BoostMissing)?;
    let boost = counts
        .get(&FieldKey::new(OWNER, BOOST))
        .ok_or("reviewed boost")?;
    let heat = counts
        .get(&FieldKey::new(OWNER, HEAT))
        .ok_or("unrelated heat")?;
    assert_eq!(boost.declaration, DeclarationState::ExpectedAbsent);
    assert_eq!(boost.review, Review::Reviewed);
    assert_eq!(heat.declaration, DeclarationState::Missing);
    assert_eq!(accounting::regressions(&counts), 1);

    let counts = survey(&inventory, Some(&build), SourceShape::BoostDeclared)?;
    let boost = counts
        .get(&FieldKey::new(OWNER, BOOST))
        .ok_or("returned boost")?;
    assert_eq!(boost.declaration, DeclarationState::UnexpectedlyDeclared);
    assert_eq!(
        boost.present, 0,
        "declaration drift needs no populated instances"
    );
    assert_eq!(accounting::regressions(&counts), 2);

    let counts = survey(&inventory, Some(&build), SourceShape::OwnerMissing)?;
    assert_eq!(accounting::regressions(&counts), 2, "the owner must exist");
    Ok(())
}

#[test]
fn an_unknown_or_different_build_keeps_the_ordinary_declaration_requirement() -> super::Result<()> {
    let inventory = inventory_with(vec![expected_absence()])?;
    let absent_without_identity = survey(&inventory, None, SourceShape::BoostMissing)?;
    assert_eq!(accounting::regressions(&absent_without_identity), 2);
    for (coordinate, replacement) in [
        ("channel", "PTU"),
        ("branch", "other-branch"),
        ("version", "other-version"),
        ("changeNumber", "124"),
        ("buildId", "other-build"),
    ] {
        let mut claim = build_claim();
        claim
            .as_object_mut()
            .ok_or("fixture Build identity")?
            .insert(coordinate.to_owned(), json!(replacement));
        let build = serde_json::from_value(claim)?;
        let counts = survey(&inventory, Some(&build), SourceShape::BoostMissing)?;
        assert_eq!(
            counts
                .get(&FieldKey::new(OWNER, BOOST))
                .ok_or("required boost")?
                .declaration,
            DeclarationState::Missing,
        );
        assert_eq!(accounting::regressions(&counts), 2);
        let counts = survey(&inventory, Some(&build), SourceShape::BoostDeclared)?;
        assert_eq!(accounting::regressions(&counts), 1);
    }
    Ok(())
}

#[test]
fn inventory_requires_unique_complete_reasoned_absence_claims() -> super::Result<()> {
    let old: Inventory = serde_json::from_value(inventory_value())?;
    assert!(old.expected_absences.is_empty());
    old.reviewed()?;
    let duplicate = inventory_with(vec![expected_absence(), expected_absence()])?;
    assert!(duplicate.reviewed().is_err());
    for (path, replacement) in [
        ("/source/field", "unreviewed"),
        ("/build/channel", "*"),
        ("/build/buildId", ""),
        ("/reason", " "),
    ] {
        let mut claim = expected_absence();
        *claim.pointer_mut(path).ok_or("fixture claim field")? = json!(replacement);
        assert!(inventory_with(vec![claim])?.reviewed().is_err());
    }
    Ok(())
}
