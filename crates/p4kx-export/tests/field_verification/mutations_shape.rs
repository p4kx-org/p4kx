// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue};
use serde_json::{Value, json};

use super::compare::Class;
use super::fixtures::{self, GUID, PublishedFixture};
use super::path::Path;
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
    field.catalog = Path::fields(&["recordIndex", "value"]);
    fixture.snapshot.fields.push(field);
    fixture.save()?;
    let catalog = json!({
        "items": [fixtures::item(GUID, json!(1))],
        "definitions": {"commodities": {"availability": "available", "rows": [{
            "guid": sourced(json!(fixtures::OTHER)), "recordIndex": sourced(json!(2))
        }]}},
        "commodities": [fixtures::item(fixtures::OTHER, json!(9))]
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

fn sourced(value: Value) -> Value {
    json!(SourceUses::new().value(SourceValue::Present(value), &[], false))
}

#[test]
fn canonical_commodity_presentation_uses_its_sourced_identity_and_explicit_field_path()
-> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(2))])?;
    fixture
        .snapshot
        .files
        .first_mut()
        .ok_or("file mapping absent")?
        .family = Family::CommodityPresentation;
    let field = fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?;
    field.family = Family::CommodityPresentation;
    field.catalog = Path::fields(&["densityGramsPerCubicCentimetre", "value"]);
    fixture.save()?;
    let mut catalog = json!({
        "projections": {"commodityPresentation": {"availability": "available", "rows": [{
            "commodityGuid": sourced(json!(GUID)),
            "densityGramsPerCubicCentimetre": sourced(json!(2))
        }]}},
        "commodities": [fixtures::item(GUID, json!(9))]
    });
    let verify = |catalog: &Value| {
        super::published::verify(
            &fixture.load()?,
            catalog,
            &fixtures::build(),
            &[fixtures::PUBLICATION],
            &|| false,
        )
    };
    let report = verify(&catalog)?;
    assert_eq!(report.matched_guids, 1);
    assert!(
        report.succeeds(),
        "only the canonical presentation owns this comparison"
    );
    catalog["projections"]["commodityPresentation"]["rows"][0]["densityGramsPerCubicCentimetre"]
        ["value"] = json!(3);
    let report = verify(&catalog)?;
    assert_eq!(
        report
            .fields
            .values()
            .map(|summary| summary.count(Class::Different))
            .sum::<u64>(),
        1
    );
    assert!(
        !report.succeeds(),
        "a legacy root row cannot mask a changed canonical value"
    );
    catalog["projections"]["commodityPresentation"]["rows"][0]["commodityGuid"]["value"] =
        json!(fixtures::OTHER);
    assert!(
        !verify(&catalog)?.succeeds(),
        "an unrelated canonical identity cannot satisfy the source GUID"
    );
    Ok(())
}

#[test]
fn commodity_definition_and_presentation_reconcile_independent_sources_in_one_audit_scope()
-> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    fixture
        .snapshot
        .files
        .first_mut()
        .ok_or("file mapping absent")?
        .family = Family::Commodities;
    let definition = fixture
        .snapshot
        .fields
        .first_mut()
        .ok_or("field mapping absent")?;
    definition.family = Family::Commodities;
    definition.catalog = Path::fields(&["recordIndex", "value"]);
    fixture.add_file(
        "presentation.json",
        Family::CommodityPresentation,
        vec![fixtures::row(fixtures::OTHER, json!(2))],
    )?;
    let mut presentation = fixtures::field();
    presentation.family = Family::CommodityPresentation;
    presentation.catalog = Path::fields(&["densityGramsPerCubicCentimetre", "value"]);
    fixture.snapshot.fields.push(presentation);
    fixture.snapshot.selection = vec![GUID.to_owned(), fixtures::OTHER.to_owned()];
    fixture.save()?;
    let catalog = json!({
        "definitions": {"commodities": {"availability": "available", "rows": [{
            "guid": sourced(json!(GUID)), "recordIndex": sourced(json!(1))
        }]}},
        "projections": {"commodityPresentation": {"availability": "available", "rows": [{
            "commodityGuid": sourced(json!(fixtures::OTHER)),
            "densityGramsPerCubicCentimetre": sourced(json!(2))
        }]}}
    });
    let report = super::published::verify(
        &fixture.load()?,
        &catalog,
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        Family::Commodities.group(),
        Family::CommodityPresentation.group()
    );
    assert_eq!(
        report.matched_guids, 2,
        "both source datasets contribute their own membership"
    );
    assert_eq!(
        report.missing_both_guids, 0,
        "a shared audit scope cannot hide the second dataset"
    );
    assert!(report.succeeds());
    Ok(())
}
