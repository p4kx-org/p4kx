// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, Filter, Value as Original, ValueError};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_projectile_ammo_fixture.rs"]
mod fixture;

use fixture::Case;

const ROWS: &str = "/projections/weaponProjectileAmmoSelections/rows";

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
        assert!(
            proof::rows(&catalog, "/definitions/ammoDefinitions/rows")?.is_empty(),
            "filter excludes target definitions without losing the physical selection"
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
        let schema = p4kx_export::catalog_schema();
        jsonschema::validator_for(&schema)?
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        proof::closure(&catalog)?;
    }
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
