// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, Filter};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_linked_ammo_fixture.rs"]
mod fixture;

use fixture::Case;

const ROWS: &str = "/projections/weaponProjectileAmmoSelections/rows";

#[test]
fn linked_projectile_ammo_joins_both_roles_through_the_physical_magazine() -> TestResult {
    let builder = fixture::build(Case::Unique);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let selected_index = |guid| -> TestResult<u32> {
        let record = forge
            .record_by_guid(common::guid(guid).parse()?)
            .ok_or("fixture Record")?;
        Ok(record.index())
    };
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let catalog = build(Case::Unique, origin)?;
        let [primary, secondary] = proof::rows(&catalog, ROWS)? else {
            return Err("both physical projectile action rows".into());
        };
        assert!(
            proof::rows(&catalog, "/definitions/ammoDefinitions/rows")?.is_empty(),
            "filter excludes typed Ammo rows"
        );
        let magazine = selected_index(fixture::MAGAZINE)?;
        let weapon = selected_index(fixture::WEAPON)?;
        for (row, role, ammo) in [
            (primary, "primary", fixture::PRIMARY),
            (secondary, "secondary", fixture::SECONDARY),
        ] {
            proof::present(row, "ownerRecordIndex", &json!(weapon), Some(encrypted))?;
            let selection = selection(row, "selectedLinkedMagazine")?;
            let value = proof::at(selection, "/value")?;
            proof::present(value, "role", &json!(role), Some(encrypted))?;
            proof::present(
                value,
                "magazineRecordIndex",
                &json!(magazine),
                Some(encrypted),
            )?;
            proof::present(
                value,
                "ammoRecordIndex",
                &json!(selected_index(ammo)?),
                Some(encrypted),
            )?;
            proof::present(
                value,
                "containerComponentOrdinal",
                &json!(1),
                Some(encrypted),
            )?;
            check_origins(&catalog, selection, weapon, magazine)?;
        }
        let schema = p4kx_export::catalog_schema();
        jsonschema::validator_for(&schema)?
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_origins(catalog: &Value, selection: &Value, weapon: u32, magazine: u32) -> TestResult {
    let uses = proof::uses(catalog, selection)?;
    for scope in [weapon, magazine] {
        assert!(
            uses.iter().any(|used| {
                used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                    && used.pointer("/location/indices/0") == Some(&json!(scope.to_string()))
                    && used.pointer("/location/field") == Some(&json!("$fields"))
            }),
            "source union contains the declaration survey of physical Record {scope}"
        );
    }
    assert_eq!(
        proof::at(selection, "/sourceSetComplete")?,
        &json!(true),
        "complete caller and magazine dependency evidence"
    );
    Ok(())
}

#[test]
fn linked_projectile_ammo_keeps_all_unproved_routes_separate() -> TestResult {
    for (case, expected) in [
        (Case::NoContainer, "linkedMagazineWithoutContainer"),
        (Case::MultipleContainers, "ambiguousLinkedMagazineContainer"),
        (
            Case::TrailingUnreadable,
            "incompleteLinkedMagazineComponents",
        ),
        (Case::DuplicateGuid, "ambiguousMagazineReference"),
        (Case::Dangling, "danglingMagazine"),
        (Case::WrongType, "unsupportedMagazineTarget"),
        (Case::OwnSubtype, "ambiguousComponents"),
        (Case::OwnWithLink, "otherMagazineRoute"),
        (Case::Conditional, "otherMagazineRoute"),
        (Case::SelfLink, "selfLinkedMagazine"),
    ] {
        let output = build(case, DcbOrigin::Encrypted)?;
        let [primary, secondary] = proof::rows(&output, ROWS)? else {
            return Err(
                format!("both physical rejected projectile action rows for {case:?}").into(),
            );
        };
        for row in [primary, secondary] {
            let result = selection(row, expected)?;
            let value = proof::at(result, "/value")?;
            assert!(
                value.get("ammoRecordIndex").is_none(),
                "unproved route has no selected ammo: {case:?}"
            );
        }
        proof::closure(&output)?;
    }
    let output = build(Case::MissingSecondary, DcbOrigin::Encrypted)?;
    let [primary, secondary] = proof::rows(&output, ROWS)? else {
        return Err("both linked launchers preserve their individual outcomes".into());
    };
    selection(primary, "selectedLinkedMagazine")?;
    selection(secondary, "missingAmmo")?;
    proof::closure(&output)
}

#[test]
fn linked_projectile_ammo_keeps_an_unreadable_physical_dependency() -> TestResult {
    let mut bytes = fixture::build(Case::Unique).build();
    let forge = DataForge::open(&bytes)?;
    let magazine = forge
        .record_by_guid(common::guid(fixture::MAGAZINE).parse()?)
        .ok_or("fixture magazine header")?;
    let index = magazine.index();
    super::resolution::break_instance(&mut bytes, index)?;
    let filter = Filter::parse(fixture::FILTER)?;
    let output = super::resolution::diagnostic(&bytes, &filter)?;
    assert_eq!(
        proof::at(&output, "/exportHealth/status")?,
        "incomplete",
        "unreadable dependent Record is in export health"
    );
    let [primary, secondary] = proof::rows(&output, ROWS)? else {
        return Err("both readable launcher occurrences remain".into());
    };
    for row in [primary, secondary] {
        let result = selection(row, "unreadableMagazine")?;
        assert_eq!(
            proof::at(result, "/sourceSetComplete")?,
            &json!(false),
            "target body evidence is unresolved"
        );
        assert!(
            proof::uses(&output, result)?.iter().any(|used| {
                used.pointer("/location/namespace") == Some(&json!("dataforgeRecordHeader"))
                    && used.pointer("/location/indices/0") == Some(&json!(index.to_string()))
            }),
            "known physical target header is retained"
        );
    }
    proof::closure(&output)
}

fn selection<'row>(row: &'row Value, expected: &str) -> TestResult<&'row Value> {
    let selection = proof::cell(row, "selection", "present")?;
    assert_eq!(
        proof::at(selection, "/value/type")?,
        expected,
        "the observed linked route decides the typed outcome"
    );
    Ok(selection)
}

fn build(case: Case, origin: DcbOrigin) -> TestResult<Value> {
    let (_folder, archive) = common::open(&fixture::build(case), origin)?;
    let mut options = common::options();
    let filter = Filter::parse(fixture::FILTER)?;
    options.filter = Some(&filter);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let result = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(result.json())?)
}
