// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::loadouts_support::{loadout, port};

#[path = "catalog_vehicle_armor_fixture.rs"]
mod fixture;

const TABLE: &str = "/configurations/vehicleArmor/rows";

#[test]
fn vehicle_and_suit_armor_keep_independent_original_tuning() -> TestResult {
    let database = fixture::database(fixture::Layout::Complete);
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        let rows = proof::rows(&catalog, TABLE)?;
        assert_eq!(
            rows.len(),
            4,
            "each physical vehicle armour is emitted once"
        );
        let vehicle = configuration(&catalog, fixture::KNOWN)?;
        check_signatures(vehicle, encrypted)?;
        check_multiplier(vehicle, encrypted)?;
        check_resistance(vehicle, encrypted)?;
        check_suit_owners(&catalog, encrypted)?;
        check_other_multipliers(&catalog, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn a_broken_curve_link_is_diagnosed_with_its_original_guid() -> TestResult {
    let database = fixture::database(fixture::Layout::BrokenCurve);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "a malformed authored curve affects normal publication"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let original = proof::at(
        configuration(&catalog, fixture::KNOWN)?,
        "/armorDeflection/value",
    )?;
    proof::cell(original, "healthCurve", "unreadable")?;
    let raw = raw_member(original, "healthCurve")?;
    let failure = proof::at(raw, "/originalError")?;
    assert_eq!(
        failure["type"], "danglingReference",
        "the actual stored Reference disposition"
    );
    proof::present(
        failure,
        "guid",
        &json!(common::guid(fixture::MISSING)),
        Some(true),
    )?;
    proof::closure(&catalog)?;
    Ok(())
}

#[test]
fn an_invisible_armor_override_joins_its_physical_definition() -> TestResult {
    let (_folder, archive) = fixture::loadout_archive()?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let ship = loadout(&catalog, fixture::SHIP)?;
    let ports = proof::at(ship, "/ports")?;
    let attached = port(ports, "armor_mount")?;
    assert_eq!(
        attached["source"], "loadout",
        "the explicit override selects the attachment"
    );
    assert_eq!(
        attached["visibility"], "invisible",
        "the original vehicle port is invisible"
    );
    let identity = proof::at(attached, "/sourceConstraints/attachedItem")?;
    proof::present(
        identity,
        "targetGuid",
        &json!(common::guid(fixture::KNOWN)),
        Some(true),
    )?;
    let canonical = configuration(&catalog, fixture::KNOWN)?;
    assert_eq!(
        proof::at(identity, "/targetRecordIndex/value")?,
        proof::at(canonical, "/ownerRecordIndex/value")?,
        "the selected physical Record joins its configuration"
    );
    let lost = port(ports, "lost_mount")?;
    assert_eq!(
        lost["unresolved"], "dangling_reference",
        "the original unresolved placement is kept"
    );
    assert_eq!(
        lost["itemGuid"],
        json!(common::guid(fixture::MISSING)),
        "the unresolved placement retains the original target GUID"
    );
    assert!(
        proof::at(lost, "/sourceConstraints/attachedItem")?.is_null(),
        "an unresolved attachment has no selected physical owner"
    );
    Ok(())
}

fn check_signatures(row: &Value, encrypted: bool) -> TestResult {
    proof::present(
        row,
        "sourceType",
        &json!("SCItemVehicleArmorParams"),
        Some(encrypted),
    )?;
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(fixture::KNOWN)),
        Some(encrypted),
    )?;
    scalar(
        row,
        "signalInfrared",
        f32::from_bits(fixture::SIGNAL_BITS),
        encrypted,
    )?;
    scalar(row, "signalElectromagnetic", 0.0, encrypted)?;
    scalar(row, "signalCrossSection", 1.0, encrypted)?;
    Ok(())
}

fn check_multiplier(row: &Value, encrypted: bool) -> TestResult {
    let multiplier = proof::at(row, "/damageMultiplier/value")?;
    assert_eq!(
        multiplier["kind"], "info",
        "the actual DamageInfo alternative"
    );
    damage_words(multiplier, 0.0, encrypted)
}

fn check_resistance(row: &Value, encrypted: bool) -> TestResult {
    let penetration = proof::at(row, "/armorPenetrationResistance/value")?;
    scalar(penetration, "basePenetrationReduction", 0.0, encrypted)?;
    damage_words(
        proof::at(penetration, "/penetrationAbsorptionForType/value")?,
        10.0,
        encrypted,
    )?;
    let deflection = proof::at(row, "/armorDeflection/value")?;
    damage_words(
        proof::at(deflection, "/deflectionValue/value")?,
        20.0,
        encrypted,
    )?;
    let curve = proof::at(deflection, "/healthCurve/value")?;
    proof::present(curve, "sourceType", &json!("BezierCurve"), Some(encrypted))?;
    proof::present(curve, "useLut", &json!(false), Some(encrypted))?;
    let [first, second] = proof::rows(curve, "/points/value")? else {
        return Err("the two original curve slots".into());
    };
    for slot in [first, second] {
        let point = proof::at(slot, "/value")?;
        scalar(point, "x", -0.0, encrypted)?;
        scalar(point, "y", 0.5, encrypted)?;
    }
    Ok(())
}

fn check_suit_owners(catalog: &Value, encrypted: bool) -> TestResult {
    let suit = item(catalog, fixture::SUIT)?;
    assert!(
        suit.pointer("/stats/armor").is_some_and(Value::is_object),
        "suit armour retains its own stats"
    );
    let suit_guid = common::guid(fixture::SUIT);
    assert!(
        proof::rows(catalog, TABLE)?.iter().all(|row| row
            .pointer("/ownerGuid/value")
            .and_then(Value::as_str)
            != Some(suit_guid.as_str())),
        "the suit-only Entity has no vehicle armour occurrence"
    );
    let dual = configuration(catalog, fixture::BOTH)?;
    proof::present(
        dual,
        "ownerGuid",
        &json!(common::guid(fixture::BOTH)),
        Some(encrypted),
    )?;
    proof::cell(dual, "damageMultiplier", "null")?;
    assert!(
        item(catalog, fixture::BOTH)?
            .pointer("/stats/armor")
            .is_some_and(Value::is_object),
        "the same Entity retains its separate suit armour"
    );
    let vehicle = item(catalog, fixture::KNOWN)?;
    assert!(
        vehicle["stats"].is_null(),
        "vehicle armour has an independent canonical row"
    );
    Ok(())
}

fn check_other_multipliers(catalog: &Value, encrypted: bool) -> TestResult {
    let future = proof::at(
        configuration(catalog, fixture::FUTURE)?,
        "/damageMultiplier/value",
    )?;
    assert_eq!(
        future["kind"], "unknown",
        "an admitted new subtype keeps its original kind"
    );
    proof::present(
        future,
        "sourceType",
        &json!("FixtureFutureDamage"),
        Some(encrypted),
    )?;
    proof::present(
        raw_member(future, "fixtureCoefficient")?,
        "value",
        &json!({
            "type":"number", "value": {"type":"f32","rawBits":"ff800000","finite":null}
        }),
        Some(encrypted),
    )?;
    let parameters = proof::at(
        configuration(catalog, fixture::PARAMETERS)?,
        "/damageMultiplier/value",
    )?;
    assert_eq!(
        parameters["kind"], "parameters",
        "the actual DamageParams alternative"
    );
    scalar(parameters, "damageTotal", 0.0, encrypted)?;
    let macro_link = proof::at(parameters, "/damageMacro/value/value")?;
    assert_eq!(
        macro_link["status"], "dangling",
        "the original missing DamageMacro target"
    );
    proof::present(
        macro_link,
        "guid",
        &json!(common::guid(fixture::MISSING)),
        Some(encrypted),
    )?;
    Ok(())
}

fn damage_words(row: &Value, offset: f32, encrypted: bool) -> TestResult {
    proof::present(row, "sourceType", &json!("DamageInfo"), Some(encrypted))?;
    for (_, field, base) in fixture::DAMAGE_WORDS {
        scalar(row, field, base + offset, encrypted)?;
    }
    Ok(())
}

fn scalar(row: &Value, field: &str, original: f32, encrypted: bool) -> TestResult {
    proof::present(
        row,
        field,
        &json!({
            "rawBits":format!("{:08x}",original.to_bits()), "finite":original.is_finite().then_some(original),
        }),
        Some(encrypted),
    )?;
    Ok(())
}

fn raw_member<'row>(owner: &'row Value, field: &str) -> TestResult<&'row Value> {
    let root = proof::rows(owner, "/additionalFields/roots")?
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(field))
        .ok_or("original additional member")?;
    let index = proof::at(root, "/value/valueIndex")?
        .as_str()
        .ok_or("raw value index")?
        .parse::<usize>()?;
    proof::rows(owner, "/additionalFields/values")?
        .get(index)
        .ok_or_else(|| "raw field value".into())
}

fn configuration(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let owner = common::guid(guid);
    proof::rows(catalog, TABLE)?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(owner.as_str()))
        .ok_or_else(|| "original vehicle armour configuration".into())
}

fn item(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let owner = common::guid(guid);
    proof::rows(catalog, "/items")?
        .iter()
        .find(|row| row.get("guid").and_then(Value::as_str) == Some(owner.as_str()))
        .ok_or_else(|| "original item".into())
}
