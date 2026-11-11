// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Value as DataForgeValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ammo_radar_fixture.rs"]
mod fixture;

use self::fixture::{NAME_KEY, TEXT_KEY, database};

#[test]
fn radar_preserves_link_slots_and_typed_scan_values_with_independent_origins() -> TestResult {
    let (_folder, archive) = common::open(&database("Tag"), DcbOrigin::Encrypted)?;
    check_original_radar_field(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let ammo = proof::at(&catalog, "/definitions/ammoDefinitions/rows/0")?;
    let radar_cell = proof::cell(ammo, "radarParams", "present")?;
    let radar = proof::at(radar_cell, "/value")?;
    proof::present(radar, "bindingUrlPrefix", &json!("fixture:"), Some(true))?;
    check_detection(&catalog, radar)?;
    check_signature(radar)?;
    check_scan(&catalog, radar)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn radar_reference_array_rejects_a_different_original_target_declaration() -> TestResult {
    let (_folder, archive) = common::open(&database("FixtureWrongTarget"), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    assert!(
        matches!(
            build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "an incompatible declared target refuses complete publication"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let radar = proof::at(
        &catalog,
        "/definitions/ammoDefinitions/rows/0/radarParams/value",
    )?;
    let rejected = proof::cell(radar, "detectionTags", "unsupported")?;
    let uses = proof::uses(&catalog, rejected)?;
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeStruct")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("name")
        }),
        "the rejected slot array keeps the original declared target name"
    );
    proof::closure(&catalog)
}

fn check_original_radar_field(archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let ammo = forge.record(0).ok_or("original Ammo Record")?;
    let tree = ammo.tree()?;
    assert!(
        matches!(
            tree.field("radarObjectParams"),
            Some(DataForgeValue::Struct(value)) if value.type_name() == "SSCSignatureSystemParams"
        ),
        "the exact authored Ammo pointer supplies the signature-system value"
    );
    assert!(
        tree.field("radarParams").is_none(),
        "the fixture declares only the original source name"
    );
    Ok(())
}

fn check_detection(catalog: &Value, radar: &Value) -> TestResult {
    let rows = proof::rows(radar, "/detectionTags/value")?;
    let [first, null, dangling, other, last] = rows else {
        return Err("expected five authored detection-tag slots".into());
    };
    assert_eq!(
        proof::at(first, "/value/status")?,
        "present",
        "the first original Tag link resolves"
    );
    assert_eq!(
        proof::at(null, "/state")?,
        "null",
        "the stored null slot is preserved"
    );
    assert_eq!(
        proof::at(dangling, "/value/status")?,
        "dangling",
        "the missing Tag remains a dangling link"
    );
    assert_eq!(
        proof::at(other, "/value/status")?,
        "unexpectedTargetType",
        "the original wrong-family target remains distinguishable"
    );
    assert_eq!(
        proof::at(last, "/value/status")?,
        "present",
        "the repeated Tag link resolves independently"
    );
    assert_eq!(
        proof::at(first, "/value/guid/value")?,
        proof::at(last, "/value/guid/value")?,
        "the two original slots refer to the same Tag"
    );
    assert_ne!(
        proof::at(first, "/sourceUsesRefs")?,
        proof::at(last, "/sourceUsesRefs")?,
        "repeated references retain independent original array slots"
    );
    let uses = proof::uses(catalog, dangling)?;
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some("$error.guid")
        }),
        "the missing target keeps the original dangling reference word"
    );
    Ok(())
}

fn check_signature(radar: &Value) -> TestResult {
    let words = proof::rows(
        radar,
        "/radarProperties/value/baseSignatureParams/value/value/signatures/value",
    )?;
    let [signed_zero, nan] = words else {
        return Err("expected both original signature slots".into());
    };
    assert_eq!(
        proof::at(signed_zero, "/value/rawBits")?,
        "80000000",
        "the original sign bit of zero survives"
    );
    assert_eq!(
        proof::at(nan, "/value/rawBits")?,
        "7fc01234",
        "the original NaN payload bits survive"
    );
    assert_eq!(
        proof::at(nan, "/value/finite")?,
        &Value::Null,
        "nonfinite input has no finite interpretation"
    );
    assert_eq!(
        proof::at(nan, "/containsEncryptedSource")?,
        &json!(true),
        "the original number came from the encrypted Entry"
    );
    assert_eq!(
        proof::at(
            radar,
            "/radarProperties/value/crossSectionParams/value/type"
        )?,
        "manual",
        "the concrete original cross section selects manual"
    );
    Ok(())
}

fn check_scan(catalog: &Value, radar: &Value) -> TestResult {
    let info = proof::at(radar, "/embeddedScanInfo/value")?;
    let datum = proof::at(
        info,
        "/scanProcedures/value/0/value/customDatas/value/0/value",
    )?;
    assert_eq!(
        proof::at(datum, "/type")?,
        "direct",
        "the original scalar scan variant remains direct"
    );
    proof::present(datum, "value", &json!(0), Some(true))?;
    let [array, float, locale] = proof::rows(info, "/values")? else {
        return Err("expected local scan array and both ordered descendants".into());
    };
    assert_eq!(
        proof::at(array, "/type")?,
        "array",
        "the original array remains a typed scan value"
    );
    assert_eq!(
        proof::at(array, "/value/value/0/value")?,
        &json!(1),
        "the first original child keeps its local index"
    );
    assert_eq!(
        proof::at(array, "/value/value/1/value")?,
        &json!(2),
        "the second original child keeps its local index"
    );
    assert_eq!(
        proof::at(float, "/type")?,
        "float",
        "the original float child retains its variant"
    );
    assert_eq!(
        proof::at(float, "/value/value/rawBits")?,
        "3f800000",
        "the original float child retains its exact word"
    );
    assert_eq!(
        proof::at(locale, "/type")?,
        "locale",
        "the original locale child retains its variant"
    );
    for (cell, key) in [
        (proof::at(datum, "/name/value")?, NAME_KEY),
        (proof::at(locale, "/value/value")?, TEXT_KEY),
    ] {
        let row = common::text_row(catalog, "gameTexts", cell)?;
        proof::present(row, "key", &json!(key), Some(true))?;
        assert!(
            proof::rows(catalog, "/texts/publicNames")?
                .iter()
                .all(|name| name.pointer("/key/value") != Some(&json!(key))),
            "protected scan text stays in its own domain"
        );
    }
    Ok(())
}
