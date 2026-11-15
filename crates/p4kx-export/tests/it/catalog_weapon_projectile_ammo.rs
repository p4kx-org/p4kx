// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, Filter, Value as Original, ValueError};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_countermeasure_emitters.rs"]
mod countermeasure_emitters;
#[path = "catalog_weapon_projectile_ammo_fixture.rs"]
mod fixture;

use fixture::Case;

const ROWS: &str = "/projections/weaponProjectileAmmoSelections/rows";
const COMBAT_ROWS: &str = "/projections/weaponCombatBases/rows";

#[test]
fn projectile_ammo_selects_each_original_role_with_complete_physical_evidence() -> TestResult {
    let builder = fixture::build(Case::Unique);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let marker = |origin| matches!(origin, DcbOrigin::Encrypted);
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted] {
        let filter =
            Filter::new("libs/foundry/records/entities/scitem/ships/fixture_projectile_own_route");
        let catalog = build(Case::Unique, origin, Some(&filter))?;
        let [required] = proof::rows(&catalog, "/definitions/ammoDefinitions/rows")? else {
            return Err("one retained original primary dependency".into());
        };
        assert_eq!(proof::at(required, "/selection")?, "dependency");
        assert_eq!(
            proof::at(required, "/guid/value")?,
            &json!(common::guid(fixture::PRIMARY))
        );
        let [primary, secondary] = proof::rows(&catalog, ROWS)? else {
            return Err("both original projectile actions".into());
        };
        for (row, role, guid) in [
            (primary, "primary", fixture::PRIMARY),
            (secondary, "secondary", fixture::SECONDARY),
        ] {
            assert_selected(&catalog, row, &forge, role, guid, marker(origin))?;
        }
        let combat = proof::rows(&catalog, COMBAT_ROWS)?;
        let [primary, secondary] = combat else {
            return Err("both authored combat modes".into());
        };
        assert_eq!(proof::at(primary, "/model/value/type")?, "nominalBullet");
        assert_eq!(
            proof::at(secondary, "/model/value/type")?,
            "ammoDefinitionUnavailable"
        );
        let schema = p4kx_export::catalog_schema();
        jsonschema::validator_for(&schema)?
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn projectile_combat_keeps_alpha_and_rate_independent_with_exact_origins() -> TestResult {
    let catalog = build(Case::Unique, DcbOrigin::Encrypted, None)?;
    let [primary, secondary] = proof::rows(&catalog, COMBAT_ROWS)? else {
        return Err("two distinct nominal projectile actions".into());
    };
    for (row, role, rate) in [(primary, "primary", 120), (secondary, "secondary", 0)] {
        assert_original_action(&catalog, row, role)?;
        let model = proof::at(row, "/model/value")?;
        assert_eq!(proof::at(model, "/type")?, "nominalBullet");
        proof::present(
            proof::at(model, "/values")?,
            "fireRateRpm",
            &json!(rate),
            Some(true),
        )?;
        proof::present(
            proof::at(model, "/values")?,
            "projectilesPerDischarge",
            &json!(3),
            Some(true),
        )?;
        proof::present(
            proof::at(model, "/values/damage/value/alpha")?,
            "energy",
            &json!(9.0),
            Some(true),
        )?;
    }
    let alpha = proof::at(primary, "/model/value/values/damage/value/alpha")?;
    let dps = proof::at(primary, "/model/value/values/damage/value/nominalDps")?;
    proof::present(alpha, "physical", &json!(3.0), Some(true))?;
    proof::present(dps, "physical", &json!(6.0), Some(true))?;
    proof::present(dps, "energy", &json!(18.0), Some(true))?;
    proof::present(dps, "distortion", &json!(0.0), Some(true))?;
    proof::cell(dps, "thermal", "absent")?;
    proof::present(
        proof::at(secondary, "/model/value/values/damage/value/alpha")?,
        "physical",
        &json!(6.0),
        Some(true),
    )?;
    proof::cell(
        proof::at(secondary, "/model/value/values/damage/value/nominalDps")?,
        "physical",
        "invalid",
    )?;
    assert_eq!(proof::at(primary, "/formulaVersion")?, &json!(1));
    proof::present(primary, "modeIndex", &json!(0), Some(true))?;
    proof::present(secondary, "modeIndex", &json!(1), Some(true))?;
    assert_eq!(
        proof::at(primary, "/units/nominalFlightDistance")?,
        "metres"
    );
    assert_combat_origins(&catalog, primary, dps)?;
    proof::closure(&catalog)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn projectile_combat_keeps_original_mode_positions_for_unsupported_and_unreadable_slots()
-> TestResult {
    let catalog = build(Case::MixedModes, DcbOrigin::Encrypted, None)?;
    let rows = proof::rows(&catalog, COMBAT_ROWS)?;
    let expected = [
        "ammoSelectionUnavailable",
        "ammoSelectionUnavailable",
        "actionUnavailable",
        "burst",
        "beam",
        "charge",
        "conditional",
    ];
    assert_eq!(rows.len(), expected.len());
    for (index, (row, kind)) in rows.iter().zip(expected).enumerate() {
        let mode = proof::cell(row, "modeIndex", "present")?;
        assert_eq!(proof::at(mode, "/value")?, &json!(index));
        assert_eq!(proof::at(mode, "/containsEncryptedSource")?, &json!(true));
        assert_eq!(proof::at(row, "/model/value/type")?, kind);
        assert_eq!(proof::at(row, "/ammunitionRecordIndex")?, &Value::Null);
    }
    assert_eq!(proof::at(&rows[2], "/actionSourceType")?, &Value::Null);
    proof::closure(&catalog)
}

fn assert_combat_origins(catalog: &Value, primary: &Value, dps: &Value) -> TestResult {
    let distance = proof::at(primary, "/model/value/values")?;
    proof::present(distance, "nominalFlightDistance", &json!(25.0), Some(true))?;
    let metric = proof::cell(distance, "nominalFlightDistance", "present")?;
    let ammo_index = proof::at(primary, "/ammunitionRecordIndex/value")?;
    let ammo = proof::rows(catalog, "/definitions/ammoDefinitions/rows")?
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(ammo_index))
        .ok_or("selected ammo definition")?;
    for field in ["speed", "lifetime"] {
        assert_uses(metric, proof::cell(ammo, field, "present")?)?;
    }
    let physical = proof::cell(dps, "physical", "present")?;
    let channels = proof::at(
        ammo,
        "/projectileParams/value/kind/value/damage/value/value",
    )?;
    assert_uses(physical, proof::cell(channels, "physical", "present")?)?;
    let energy = proof::cell(channels, "energy", "present")?;
    let physical_sources = proof::rows(physical, "/sourceUsesRefs")?;
    assert!(
        proof::rows(energy, "/sourceUsesRefs")?
            .iter()
            .any(|source| !physical_sources.contains(source)),
        "the damage cell retains independent channel evidence"
    );
    for field in [
        "projectilesPerDischarge",
        "launcherDamageMultiplier",
        "fireRateRpm",
    ] {
        assert_uses(physical, proof::cell(distance, field, "present")?)?;
    }
    Ok(())
}

fn assert_uses(derived: &Value, input: &Value) -> TestResult {
    let derived = proof::rows(derived, "/sourceUsesRefs")?;
    let input = proof::rows(input, "/sourceUsesRefs")?;
    assert!(
        !input.is_empty() && input.iter().all(|source| derived.contains(source)),
        "derived metric retains the exact source uses of its input"
    );
    Ok(())
}

fn assert_selected(
    catalog: &Value,
    row: &Value,
    forge: &DataForge<'_>,
    role: &str,
    guid: u128,
    encrypted: bool,
) -> TestResult {
    let owner = forge
        .record_by_guid(common::guid(fixture::WEAPON).parse()?)
        .ok_or("original physical owner")?;
    proof::present(
        row,
        "ownerRecordIndex",
        &json!(owner.index()),
        Some(encrypted),
    )?;
    proof::present(row, "componentOrdinal", &json!(1), Some(encrypted))?;
    let target = forge
        .record_by_guid(common::guid(guid).parse()?)
        .ok_or("original physical ammo definition")?;
    let selection = proof::cell(row, "selection", "present")?;
    let selected = proof::at(selection, "/value")?;
    assert_eq!(
        proof::at(selected, "/type")?,
        "selected",
        "uniquely proved own route"
    );
    proof::present(selected, "role", &json!(role), Some(encrypted))?;
    proof::present(
        selected,
        "containerComponentOrdinal",
        &json!(2),
        Some(encrypted),
    )?;
    proof::present(
        selected,
        "ammoRecordIndex",
        &json!(target.index()),
        Some(encrypted),
    )?;
    assert_eq!(
        proof::at(selection, "/containsEncryptedSource")?,
        &json!(encrypted),
        "the selection joins physical input origins"
    );
    assert_eq!(
        proof::at(selection, "/sourceSetComplete")?,
        &json!(true),
        "the complete route survey is included"
    );
    assert_selected_origins(catalog, selection, selected)?;
    assert_original_action(catalog, row, role)
}

fn assert_selected_origins(catalog: &Value, selection: &Value, selected: &Value) -> TestResult {
    let uses = proof::uses(catalog, selection)?;
    for coordinate in ["containerComponentOrdinal", "ammoRecordIndex"] {
        let cell = proof::cell(selected, coordinate, "present")?;
        assert_eq!(
            proof::at(cell, "/sourceUsesRefs")?,
            proof::at(selection, "/sourceUsesRefs")?,
            "each projected coordinate carries the complete decision evidence"
        );
    }
    for (namespace, field) in [
        ("dataforgeStructDeclaration", "parent"),
        ("dataforgeValue", "$isArray"),
        ("dataforgeValue", "$fields"),
        ("dataforgeRecordDeclarations", "$fields"),
        ("dataforgeRecordHeader", "recordIndex"),
    ] {
        assert!(
            uses.iter()
                .any(
                    |source| source.pointer("/location/namespace") == Some(&json!(namespace))
                        && source.pointer("/location/field") == Some(&json!(field))
                ),
            "joined origin includes {namespace}.{field}"
        );
    }
    Ok(())
}

fn assert_original_action(catalog: &Value, row: &Value, role: &str) -> TestResult {
    let [weapon] = proof::rows(catalog, "/configurations/weaponActions/rows")? else {
        return Err("one authored physical weapon".into());
    };
    let [first, second] = proof::rows(weapon, "/actionNodes")? else {
        return Err("two distinct original action occurrences".into());
    };
    let action = if role == "primary" { first } else { second };
    assert_eq!(
        proof::at(row, "/actionSourceValueOrdinal")?,
        proof::at(action, "/sourceValueOrdinal")?,
        "the projection joins the original authored node identity"
    );
    Ok(())
}

#[test]
fn projectile_ammo_keeps_nonunique_and_unavailable_routes_explicit() -> TestResult {
    for (case, expected) in [
        (Case::SecondWeapon, "ambiguousComponents"),
        (Case::CompetingContainerSubtype, "ambiguousComponents"),
        (Case::UnreadableComponent, "incompleteComponents"),
        (Case::NoContainer, "noAdmittedOwnContainer"),
        (Case::IgnoredPrimary, "otherMagazineRoute"),
        (Case::LinkedMagazine, "otherMagazineRoute"),
        (Case::SecondaryMagazine, "otherMagazineRoute"),
    ] {
        let catalog = build(case, DcbOrigin::Encrypted, None)?;
        let rows = proof::rows(&catalog, ROWS)?;
        assert!(
            !rows.is_empty(),
            "source-backed unselected projectile rows for {case:?}"
        );
        for row in rows {
            check_status(row, expected)?;
        }
        proof::closure(&catalog)?;
    }
    for (case, expected) in [
        (Case::UnsupportedRole, "unsupportedProjectileType"),
        (Case::DanglingAmmo, "danglingAmmo"),
        (Case::MissingAmmo, "missingAmmo"),
        (Case::DerivedAmmo, "unsupportedAmmoTarget"),
    ] {
        let catalog = build(case, DcbOrigin::Encrypted, None)?;
        let [primary, secondary] = proof::rows(&catalog, ROWS)? else {
            return Err("both original actions survive a failed second selection".into());
        };
        check_status(primary, "selected")?;
        check_status(secondary, expected)?;
    }
    Ok(())
}

#[test]
fn projectile_ammo_preserves_repeated_physical_action_as_ambiguous() -> TestResult {
    let builder = fixture::build(Case::RepeatedAction);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record_by_guid(common::guid(fixture::WEAPON).parse()?)
        .ok_or("original owner")?
        .tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    let Some(Original::Struct(weapon)) = components.get(1) else {
        return Err("original weapon component".into());
    };
    let Some(Original::Array(actions)) = weapon.field("fireActions") else {
        return Err("original fire action slots".into());
    };
    assert!(
        matches!(
            actions.get(1),
            Some(Original::Error(ValueError::OwnedTwice { .. }))
        ),
        "canonical original decoder sees a second physical owner"
    );
    for (case, expected) in [
        (Case::RepeatedAction, "ambiguousAction"),
        (Case::InvalidAction, "incompleteActions"),
    ] {
        let catalog = build(case, DcbOrigin::Encrypted, None)?;
        let [row] = proof::rows(&catalog, ROWS)? else {
            return Err("one readable original action occurrence".into());
        };
        check_status(row, expected)?;
        assert_eq!(
            proof::at(row, "/selection/sourceSetComplete")?,
            &json!(false),
            "an unobserved source attempt leaves the decision incomplete"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_status(row: &Value, expected: &str) -> TestResult {
    assert_eq!(
        proof::at(row, "/selection/value/type")?,
        expected,
        "source-backed selection outcome"
    );
    Ok(())
}

fn build(case: Case, origin: DcbOrigin, filter: Option<&Filter>) -> TestResult<Value> {
    let (_folder, archive) = common::open(&fixture::build(case), origin)?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}
