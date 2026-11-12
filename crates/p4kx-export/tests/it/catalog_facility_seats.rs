// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_seat_fixture.rs"]
mod fixture;

#[test]
fn seat_and_ejection_keep_their_physical_source_and_typed_values() -> TestResult {
    let database = fixture::database(-8.0);
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let [crew, operator] = proof::rows(&catalog, "/configurations/facilities/rows")? else {
            return Err("two physical seat component occurrences".into());
        };
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        check_owner(crew, 1, "FixtureCrew", encrypted)?;
        check_owner(operator, 2, "FixtureOperator", encrypted)?;
        check_crew(crew, encrypted)?;
        proof::cell(crew, "controlTemplate", "absent")?;
        check_ejection(crew, encrypted)?;
        proof::cell(operator, "ejection", "null")?;
        proof::cell(operator, "availableOperatorModes", "null")?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_owner(row: &Value, ordinal: u32, seat: &str, encrypted: bool) -> TestResult {
    assert_eq!(row["kind"], "seat", "authored component family");
    for (field, expected) in [
        ("ownerGuid", json!(common::guid(fixture::OWNER))),
        ("ownerRecordIndex", json!(0)),
        ("componentOrdinal", json!(ordinal)),
        ("sourceType", json!(fixture::SEAT)),
        ("seatType", json!(seat)),
    ] {
        proof::present(row, field, &expected, Some(encrypted))?;
    }
    Ok(())
}

fn check_crew(row: &Value, encrypted: bool) -> TestResult {
    let actor = proof::at(row, "/actorAttachment/value")?;
    for (field, expected) in [
        ("sourceType", fixture::ATTACHMENT),
        ("type", "FixtureAttach"),
        ("boneName", "fixture_bone"),
        ("stance", "FixtureStance"),
    ] {
        proof::present(actor, field, &json!(expected), Some(encrypted))?;
    }
    for (field, value) in [
        ("minYaw", -0.0),
        ("maxYaw", 120.0),
        ("minPitch", -34.0),
        ("maxPitch", 34.0),
    ] {
        single(row, field, value, encrypted)?;
    }
    proof::present(row, "setYawPitchLimits", &json!(false), Some(encrypted))?;
    let mode = proof::at(row, "/availableOperatorModes/value")?;
    proof::present(
        mode,
        "guid",
        &json!(common::guid(fixture::MODE)),
        Some(encrypted),
    )?;
    assert_eq!(
        mode["status"], "dangling",
        "authored dangling mode Reference"
    );
    let roots = proof::rows(row, "/additionalFields/roots")?;
    let extra = roots
        .iter()
        .find(|value| value.pointer("/name/value") == Some(&json!("fixtureSeatFact")))
        .ok_or("preserved original seat member")?;
    proof::present(extra, "name", &json!("fixtureSeatFact"), Some(encrypted))?;
    let index = proof::at(extra, "/value/valueIndex")?
        .as_str()
        .ok_or("original seat value index")?
        .parse::<usize>()?;
    let value = proof::rows(row, "/additionalFields/values")?
        .get(index)
        .ok_or("original seat value")?;
    proof::present(
        value,
        "value",
        &json!({"type": "number", "value": {"type": "uint16", "value": 43}}),
        Some(encrypted),
    )?;
    assert!(
        roots
            .iter()
            .any(|value| value.pointer("/name/value") == Some(&json!("userAnimationDatabase"))),
        "the authored animation database is also retained"
    );
    Ok(())
}

fn check_ejection(row: &Value, encrypted: bool) -> TestResult {
    let ejection = proof::at(row, "/ejection/value")?;
    proof::present(
        ejection,
        "sourceType",
        &json!(fixture::EJECTION),
        Some(encrypted),
    )?;
    single(ejection, "maxLinearAcceleration", -0.0, encrypted)?;
    single(ejection, "ejectionLoopTime", 0.0, encrypted)?;
    let angles = proof::at(ejection, "/offsetAngles/value")?;
    single(angles, "x", 5.0, encrypted)?;
    single(angles, "y", -0.0, encrypted)?;
    single(angles, "z", -8.0, encrypted)?;
    let interaction = proof::at(ejection, "/ejectionInteraction/value")?;
    assert_eq!(
        proof::at(interaction, "/target/location/value")?,
        "Components[1].fixtureInteraction",
        "exact original WeakPointer instance path"
    );
    Ok(())
}

fn single(row: &Value, field: &str, original: f32, encrypted: bool) -> TestResult {
    let cell = proof::cell(row, field, "present")?;
    assert_eq!(
        proof::at(cell, "/value/rawBits")?,
        &json!(format!("{:08x}", original.to_bits())),
        "original IEEE Single bits for {field}"
    );
    assert_eq!(
        cell["containsEncryptedSource"], encrypted,
        "{field} has its actual DCB origin"
    );
    assert_eq!(
        cell["sourceSetComplete"], true,
        "{field} has complete contributors"
    );
    Ok(())
}

#[test]
fn nonfinite_ejection_offset_retains_its_exact_bits_in_a_complete_catalog() -> TestResult {
    let original = f32::from_bits(0x7fc0_1063);
    let (_directory, archive) = common::open(&fixture::database(original), DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let angles = proof::at(
        &catalog,
        "/configurations/facilities/rows/0/ejection/value/offsetAngles/value",
    )?;
    single(angles, "z", original, true)?;
    assert_eq!(
        proof::at(angles, "/z/value/finite")?,
        &Value::Null,
        "a nonfinite Single has an exact raw value"
    );
    proof::closure(&catalog)?;
    Ok(())
}

#[test]
fn absent_or_invalid_ejection_preserves_original_storage_state() -> TestResult {
    let (_, absent) = common::build(&fixture::database_with_alternative_ejection(None))?;
    let row = proof::at(&absent, "/configurations/facilities/rows/0")?;
    proof::cell(row, "ejection", "absent")?;

    let text = "fixture ejection stored as text";
    let database = fixture::database_with_alternative_ejection(Some(text));
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "invalid original storage affects publication"
    );
    let options = p4kx_export::CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let row = proof::at(&catalog, "/configurations/facilities/rows/0")?;
    proof::cell(row, "ejection", "invalid")?;
    let [original] = proof::rows(row, "/additionalFields/roots")? else {
        return Err("one retained original seat member".into());
    };
    proof::present(original, "name", &json!("ejection"), Some(true))?;
    let raw = proof::at(row, "/additionalFields/values/0")?;
    proof::present(
        raw,
        "value",
        &json!({"type": "string", "value": {"type": "gameText", "sourceText": text}}),
        Some(true),
    )?;
    proof::closure(&catalog)?;
    Ok(())
}
