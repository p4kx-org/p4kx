// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::compare::Class;
use super::fixtures::{self, GUID, PublishedFixture};
use super::published_model::Family;

#[test]
fn reviewed_baseline_paths_retain_the_published_raw_shape_and_tagged_typed_projection()
-> super::Result<()> {
    let raw = json!({
        "Item": { "reference": GUID },
        "Raw": { "Entity": { "Components": {
            "SCItemShieldGeneratorParams": { "MaxShieldHealth": 20 },
            "SCItemWeaponComponentParams": { "fireActions": {
                "SWeaponActionFireRapidParams": { "fireRate": 60 }
            }}
        }}}
    });
    let mut fixture = PublishedFixture::new(vec![raw])?;
    fixture.snapshot.fields = super::baseline::raw_mappings();
    fixture.save()?;
    let catalog = json!({ "items": [{
        "guid": GUID, "stats": {
            "shield": { "maxHealth": 20 },
            "weapon": { "fireActions": [{ "kind": "SWeaponActionFireRapidParams", "fireRate": 60 }] }
        }
    }] });
    let report = super::published::verify(
        &fixture.load()?,
        &catalog,
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        report.fields.len(),
        2,
        "both reviewed owning field mappings are exercised"
    );
    assert_eq!(
        report
            .fields
            .values()
            .map(super::compare::Summary::compared)
            .sum::<u64>(),
        2,
        "raw wrapper paths and typed tagged selectors each compare one source fact"
    );
    assert!(
        report.succeeds(),
        "baseline source-to-export paths agree on synthetic primary-source shape"
    );
    Ok(())
}

#[test]
fn repeated_placements_preserve_every_occurrence_and_each_changed_raw_value() -> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![
        fixtures::row(GUID, json!(1)),
        fixtures::row(GUID, json!(2)),
    ])?;
    fixture
        .snapshot
        .files
        .first_mut()
        .ok_or("file mapping absent")?
        .family = Family::Placements;
    fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?
        .family = Family::Placements;
    fixture.save()?;
    let catalog = json!({ "starmapObjects": [
        fixtures::item(GUID, json!(1)), fixtures::item(GUID, json!(3))
    ] });
    let report = super::published::verify(
        &fixture.load()?,
        &catalog,
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    let summary = report
        .fields
        .values()
        .next()
        .ok_or("field summary absent")?;
    assert_eq!(
        summary.compared(),
        2,
        "repeated placement rows retain their full multiplicity"
    );
    assert_eq!(
        summary.count(Class::Equal),
        1,
        "one unchanged placement remains equal"
    );
    assert_eq!(
        summary.count(Class::Different),
        1,
        "the other placement's changed value is detected"
    );
    assert!(
        !report.succeeds(),
        "one equal occurrence cannot conceal a changed repeated placement"
    );
    Ok(())
}

#[test]
fn large_integral_json_values_cannot_gain_equality_from_float_rounding() {
    let first = json!((1_u64 << 60) + 1);
    let second = json!((1_u64 << 60) + 2);
    assert_eq!(
        super::compare::values(
            Some(&first),
            Some(&second),
            &super::units::Unit::Dimensionless,
            &super::units::Unit::Dimensionless,
            &super::compare::Rule::F32RoundTrip
        ),
        Class::Unhandled,
        "values outside exact JSON-integer-to-f64 range cannot enter the float policy"
    );
}

#[test]
fn selected_guids_from_two_families_are_reconciled_in_their_own_source_sets() -> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    fixture.add_file(
        "commodity.json",
        Family::Commodities,
        vec![fixtures::row(fixtures::OTHER, json!(2))],
    )?;
    fixture.snapshot.selection = vec![GUID.to_owned(), fixtures::OTHER.to_owned()];
    let mut field = fixtures::field();
    field.family = Family::Commodities;
    fixture.snapshot.fields.push(field);
    fixture.save()?;
    let catalog = json!({
        "items": [fixtures::item(GUID, json!(1))],
        "commodities": [fixtures::item(fixtures::OTHER, json!(2))]
    });
    let report = super::published::verify(
        &fixture.load()?,
        &catalog,
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        report.missing_both_guids, 0,
        "an item identity is not required in the unrelated commodity source set"
    );
    assert_eq!(
        report
            .fields
            .values()
            .map(super::compare::Summary::compared)
            .sum::<u64>(),
        2,
        "each family contributes only its actual owning raw facts"
    );
    assert!(
        report.succeeds(),
        "independent selected kinds do not create false missing-source failures"
    );
    Ok(())
}
