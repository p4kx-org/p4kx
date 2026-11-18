// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_export::{ShipBaseOptions, build_ship_base};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

#[path = "catalog_ship_aggregate_fixture.rs"]
mod fixture;
use fixture::Case;

#[test]
fn actual_parts_conversions_and_eligibility_close_each_immediate_metric() -> TestResult {
    let (_folder, archive) = fixture::open(Case::Parts)?;
    let catalog = fixture::catalog(&archive)?;
    let row = one(&catalog)?;
    let mass = proof::present(row, "mass", &json!(12.0), Some(true))?;
    let basis = proof::present(row, "massBasis", &json!("parts"), Some(true))?;
    let hull = proof::present(row, "hullHealth", &json!(8.0), Some(true))?;
    proof::present(
        row,
        "vehicleDefinition",
        &json!("Scripts/Entities/Vehicles/Implementations/Xml/AggregateShip.xml"),
        Some(false),
    )?;
    assert_eq!(
        conversion_fields(&catalog, mass, fixture::XML)?,
        BTreeSet::from(["mass".to_owned()])
    );
    assert_eq!(
        conversion_fields(&catalog, hull, fixture::XML)?,
        BTreeSet::from(["damageMax".to_owned()])
    );
    assert!(
        proof::uses(&catalog, basis)?
            .iter()
            .all(|used| used["role"] == "selector")
    );
    for carrier in [mass, basis, hull] {
        for used in proof::uses(&catalog, carrier)? {
            let source = proof::source(&catalog, used)?;
            let path = source
                .pointer("/path/value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dcb"))
            {
                assert_ne!(
                    used["role"], "conversion",
                    "unused Physics is not a Parts contributor"
                );
            }
            assert!(
                !Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ini")),
                "Ship aggregates use no locale payload"
            );
        }
    }
    parity(&archive, row)?;
    proof::closure(&catalog)
}

#[test]
fn known_empty_vehicle_and_physics_fallback_have_separate_complete_origins() -> TestResult {
    for case in [Case::Physics, Case::Empty] {
        let (_folder, archive) = fixture::open(case)?;
        let catalog = fixture::catalog(&archive)?;
        let row = one(&catalog)?;
        if matches!(case, Case::Physics) {
            proof::present(row, "mass", &json!(42.0), Some(false))?;
            proof::present(row, "massBasis", &json!("physics"), Some(false))?;
        } else {
            absent(row, "mass", Some(true), true)?;
            absent(row, "massBasis", Some(true), true)?;
        }
        absent(row, "hullHealth", Some(matches!(case, Case::Empty)), true)?;
        parity(&archive, row)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn actual_unreadable_vehicle_inputs_are_not_reported_as_absent_values() -> TestResult {
    for case in [
        Case::Missing,
        Case::Malformed,
        Case::Unreadable,
        Case::BrokenPatch,
    ] {
        let (_folder, archive) = fixture::open(case)?;
        let catalog = fixture::catalog(&archive)?;
        let row = one(&catalog)?;
        for field in ["mass", "massBasis", "hullHealth"] {
            let cell = proof::cell(row, field, "unreadable")?;
            assert_eq!(
                cell["sourceSetComplete"], false,
                "unseen configured input is explicit"
            );
        }
        parity(&archive, row)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn invalid_authored_numbers_keep_their_original_sources() -> TestResult {
    for case in [Case::InvalidValues, Case::InvalidValuesWithPhysics] {
        let (_folder, archive) = fixture::open(case)?;
        let catalog = fixture::catalog(&archive)?;
        let row = one(&catalog)?;
        invalid_number(row, "hullHealth")?;
        if matches!(case, Case::InvalidValuesWithPhysics) {
            proof::present(row, "mass", &json!(42.0), Some(true))?;
            proof::present(row, "massBasis", &json!("physics"), Some(true))?;
        } else {
            invalid_number(row, "mass")?;
            invalid_number(row, "massBasis")?;
        }
        parity(&archive, row)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn invalid_number(row: &Value, field: &str) -> TestResult {
    let cell = proof::cell(row, field, "invalid")?;
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the invalid input is encrypted"
    );
    assert_eq!(
        cell["sourceSetComplete"], true,
        "the invalid input still has original source coordinates"
    );
    Ok(())
}

#[test]
fn available_numbers_survive_genuinely_unreadable_optional_inputs() -> TestResult {
    for case in [Case::UnreadableWithPhysics, Case::BrokenPatchWithValues] {
        let (_folder, archive) = fixture::open(case)?;
        let catalog = fixture::catalog(&archive)?;
        let row = one(&catalog)?;
        let fallback = matches!(case, Case::UnreadableWithPhysics);
        let mass = proof::cell(row, "mass", "present")?;
        assert_eq!(
            mass["value"],
            if fallback { json!(42.0) } else { json!(12.0) }
        );
        assert_eq!(mass["sourceSetComplete"], false);
        let basis = proof::cell(row, "massBasis", "present")?;
        assert_eq!(
            basis["value"],
            if fallback {
                json!("physics")
            } else {
                json!("parts")
            }
        );
        assert_eq!(basis["sourceSetComplete"], false);
        let hull = proof::cell(
            row,
            "hullHealth",
            if fallback { "unreadable" } else { "present" },
        )?;
        if !fallback {
            assert_eq!(hull["value"], 8.0);
        }
        assert_eq!(hull["sourceSetComplete"], false);
        parity(&archive, row)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn original_coordinate_loss_preserves_known_values_and_known_absence() -> TestResult {
    for case in [Case::RepairedWithMass, Case::RepairedWithoutMass] {
        let (_folder, archive) = fixture::open(case)?;
        let catalog = fixture::catalog(&archive)?;
        let row = one(&catalog)?;
        for field in ["mass", "massBasis", "hullHealth"] {
            let present = matches!(case, Case::RepairedWithMass) && field != "hullHealth";
            let cell = proof::cell(row, field, if present { "present" } else { "absent" })?;
            assert_eq!(
                cell["sourceSetComplete"], false,
                "only the original coordinates are incomplete"
            );
            assert_eq!(
                cell["containsEncryptedSource"], true,
                "known encrypted XML origin survives"
            );
        }
        if matches!(case, Case::RepairedWithMass) {
            assert_eq!(row["mass"]["value"], 2.0);
            assert_eq!(row["massBasis"]["value"], "parts");
        }
        parity(&archive, row)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn cached_same_guid_same_path_callers_keep_their_own_physics_operands() -> TestResult {
    let (_folder, archive) = fixture::open(Case::Aliases)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let records: Vec<_> = forge.records().collect();
    let [first, second] = records.as_slice() else {
        return Err("two physical originals".into());
    };
    assert_eq!(first.guid(), second.guid());
    assert_eq!(first.name()?, second.name()?);
    assert_eq!(first.record_path()?, second.record_path()?);
    assert_ne!(first.index(), second.index());
    let catalog = fixture::catalog(&archive)?;
    let rows = proof::rows(&catalog, "/ships")?;
    assert_eq!(rows.len(), 2);
    for ((row, original), expected) in rows.iter().zip(records).zip([10.0, 20.0]) {
        let mass = proof::present(row, "mass", &json!(expected), Some(true))?;
        proof::present(row, "massBasis", &json!("physics"), Some(true))?;
        assert!(conversion_fields(&catalog, mass, fixture::XML)?.is_empty());
        let mut conversion = false;
        for used in proof::uses(&catalog, mass)? {
            if used
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                .is_some_and(|namespace| {
                    matches!(namespace, "dataforgeValue" | "dataforgeRecordHeader")
                })
            {
                assert_eq!(
                    used.pointer("/location/indices/0"),
                    Some(&json!(original.index().to_string())),
                    "cached evidence cites this physical caller"
                );
                conversion |= used["role"] == "conversion";
            }
        }
        assert!(
            conversion,
            "the actual original Physics operand contributes"
        );
    }
    proof::closure(&catalog)
}

#[test]
fn a_positive_parts_value_rounding_to_zero_remains_present_with_its_basis() -> TestResult {
    let (_folder, archive) = fixture::open(Case::RoundedZero)?;
    let catalog = fixture::catalog(&archive)?;
    let row = one(&catalog)?;
    proof::present(row, "mass", &json!(0.0), Some(true))?;
    proof::present(row, "massBasis", &json!("parts"), Some(true))?;
    parity(&archive, row)?;
    proof::closure(&catalog)
}

fn one(catalog: &Value) -> TestResult<&Value> {
    let [row] = proof::rows(catalog, "/ships")? else {
        return Err("one actual Ship row".into());
    };
    Ok(row)
}

fn absent(row: &Value, field: &str, marker: Option<bool>, complete: bool) -> TestResult {
    let cell = proof::cell(row, field, "absent")?;
    assert_eq!(
        cell["containsEncryptedSource"],
        json!(marker),
        "{field} retains the known source encryption"
    );
    assert_eq!(
        cell["sourceSetComplete"], complete,
        "{field} retains its actual input completeness"
    );
    Ok(())
}

fn conversion_fields(catalog: &Value, cell: &Value, path: &str) -> TestResult<BTreeSet<String>> {
    let mut fields = BTreeSet::new();
    for used in proof::uses(catalog, cell)? {
        let source = proof::source(catalog, used)?;
        let actual = source
            .pointer("/path/value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace('\\', "/");
        if actual == path && used["role"] == "conversion" {
            fields.insert(
                used.pointer("/location/field")
                    .and_then(Value::as_str)
                    .ok_or("original conversion coordinate")?
                    .to_owned(),
            );
        }
    }
    Ok(fields)
}

fn parity(archive: &p4kx_extraction::Archive, row: &Value) -> TestResult {
    let built = build_ship_base(archive, &ShipBaseOptions::new(UNIX_EPOCH))?;
    let [base] = built.ships() else {
        return Err("one standalone ShipBase".into());
    };
    assert_eq!(
        proof::at(row, "/mass/value")?.as_f64(),
        base.mass.as_ref().map(|mass| mass.kg),
        "Catalog mass agrees with the standalone ShipBase calculation"
    );
    assert_eq!(
        proof::at(row, "/hullHealth/value")?.as_f64(),
        base.hull.as_ref().map(|hull| hull.health),
        "Catalog hull health agrees with the standalone ShipBase calculation"
    );
    let basis = base
        .mass
        .as_ref()
        .map(|mass| serde_json::to_value(mass.basis))
        .transpose()?
        .unwrap_or(Value::Null);
    assert_eq!(
        proof::at(row, "/massBasis/value")?,
        &basis,
        "Catalog mass basis agrees with the standalone ShipBase calculation"
    );
    Ok(())
}

#[path = "catalog_ship_source_sdk.rs"]
mod source_sdk;

#[path = "catalog_ship_source_selection_sdk.rs"]
mod source_selection_sdk;

#[path = "catalog_ship_source_texts_sdk.rs"]
mod source_texts_sdk;
