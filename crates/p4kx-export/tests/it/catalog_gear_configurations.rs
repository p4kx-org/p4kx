// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::gear_fixture::{self, ARMOR, ARMOR_DANGLING, CLOTHING, RADAR, WRAPPED_TRACTOR};
use crate::health_origin_assertions as proof;

#[test]
fn a_filtered_suit_keeps_its_physical_resistance_macro_dependency() -> TestResult {
    let filter = Filter::new("*/gear/armor");
    let catalog = catalog(Some(&filter))?;
    let suit = owner(&catalog, "suitArmor", 0, ARMOR, 1)?;
    number(suit, "integrityMilestoneToBreak", 0.1)?;
    only_additional_field(suit, "protectedBodyParts")?;
    let macro_ref = proof::at(suit, "/damageResistance/value")?;
    assert_eq!(
        proof::at(macro_ref, "/status")?,
        "present",
        "resolved macro link"
    );
    proof::present(
        macro_ref,
        "targetType",
        &json!("DamageResistanceMacro"),
        Some(true),
    )?;
    let macro_row = proof::at(&catalog, "/definitions/damageResistanceMacros/rows/0")?;
    let macro_source = proof::at(macro_row, "/source")?;
    proof::present(macro_source, "recordIndex", &json!(0), Some(true))?;
    assert_eq!(
        proof::at(macro_source, "/recordIndex/value")?,
        proof::at(macro_ref, "/targetRecordIndex/value")?,
        "exact physical target of the suit Reference",
    );
    let resistance = proof::at(macro_row, "/damageResistance/value")?;
    proof::present(resistance, "ignoreMeleeDamage", &json!(false), Some(true))?;
    let physical = proof::at(resistance, "/physical/value")?;
    number(physical, "multiplier", 0.6)?;
    number(physical, "threshold", 0.0)?;
    number(physical, "damageCap", 0.0)?;
    number(physical, "damageCapLimit", 0.0)?;
    let energy = proof::at(resistance, "/energy/value")?;
    number(energy, "multiplier", 0.75)?;
    number(energy, "threshold", 2.0)?;
    proof::cell(resistance, "stun", "absent")?;
    proof::closure(&catalog)
}

#[test]
fn a_dangling_suit_reference_never_admits_another_macros_values() -> TestResult {
    let filter = Filter::new("*/gear/armor_dangling");
    let catalog = catalog(Some(&filter))?;
    let suit = owner(&catalog, "suitArmor", 0, ARMOR_DANGLING, 2)?;
    let macro_ref = proof::at(suit, "/damageResistance/value")?;
    assert_eq!(
        proof::at(macro_ref, "/status")?,
        "dangling",
        "missing target"
    );
    proof::cell(macro_ref, "targetRecordIndex", "absent")?;
    assert_eq!(
        proof::at(&catalog, "/definitions/damageResistanceMacros/rows")?,
        &json!([]),
        "no unrelated definition passes the owner-only filter",
    );
    proof::closure(&catalog)
}

#[test]
fn repeated_macro_guids_keep_distinct_physical_definitions() -> TestResult {
    let catalog = catalog(None)?;
    let [first, duplicate] = proof::rows(&catalog, "/definitions/damageResistanceMacros/rows")?
    else {
        return Err("both physical macro records remain available".into());
    };
    assert_eq!(
        proof::at(first, "/source/guid/value")?,
        proof::at(duplicate, "/source/guid/value")?,
        "both original definitions share a GUID",
    );
    assert_ne!(
        proof::at(first, "/source/recordIndex/value")?,
        proof::at(duplicate, "/source/recordIndex/value")?,
        "physical identity distinguishes the two definitions",
    );
    number(
        proof::at(first, "/damageResistance/value/physical/value")?,
        "multiplier",
        0.6,
    )?;
    number(
        proof::at(duplicate, "/damageResistance/value/physical/value")?,
        "multiplier",
        0.25,
    )?;
    let suit = owner(&catalog, "suitArmor", 0, ARMOR, 1)?;
    assert_eq!(
        proof::at(suit, "/damageResistance/value/targetRecordIndex/value")?,
        proof::at(first, "/source/recordIndex/value")?,
        "the authored Reference resolves to the first physical Record",
    );
    proof::closure(&catalog)
}

#[test]
fn clothing_and_radar_keep_original_structure_values_and_provenance() -> TestResult {
    let catalog = catalog(None)?;
    let clothing = owner(&catalog, "clothing", 0, CLOTHING, 3)?;
    let temperature = proof::at(clothing, "/temperatureResistance/value")?;
    number(temperature, "minResistance", -70.0)?;
    number(temperature, "maxResistance", 100.0)?;
    let radiation = proof::at(clothing, "/radiationResistance/value")?;
    number(radiation, "maximumRadiationCapacity", 26_800.0)?;
    number(radiation, "radiationDissipationRate", 145.8)?;
    number(
        proof::at(clothing, "/flight/value")?,
        "gForceResistance",
        -0.5,
    )?;
    let radar = owner(&catalog, "itemRadars", 0, RADAR, 4)?;
    only_additional_field(radar, "forceActiveAIControlled")?;
    let aim = proof::at(radar, "/aimAssist/value")?;
    number(aim, "distanceMinAssignment", 780.0)?;
    number(aim, "distanceMaxAssignment", 1028.5)?;
    number(aim, "outsideRangeBufferDistance", 60.0)?;
    number(
        proof::at(radar, "/pingProperties/value")?,
        "cooldownTime",
        2.5,
    )?;
    assert_eq!(
        proof::at(&catalog, "/configurations/radarOptions/rows")?,
        &json!([]),
        "per-device radar data does not create global mode rows",
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn a_wrapped_tractor_beam_has_exact_original_action_inputs_and_residual_fields() -> TestResult {
    let filter = Filter::new("*/gear/wrapped_tractor");
    let catalog = catalog(Some(&filter))?;
    let weapon = owner(&catalog, "weaponActions", 0, WRAPPED_TRACTOR, 8)?;
    let [beam, toggle, tractor] = proof::rows(weapon, "/actionNodes")? else {
        return Err("a direct beam, a toggle and its nested tractor beam".into());
    };
    assert_eq!(proof::at(beam, "/kind/type")?, "beam");
    assert_eq!(proof::at(toggle, "/kind/type")?, "toggle");
    assert_eq!(proof::at(toggle, "/kind/value/weaponAction/value")?, 2);
    assert_eq!(proof::at(tractor, "/kind/type")?, "tractorBeam");
    proof::present(
        tractor,
        "sourceType",
        &json!("SWeaponActionFireTractorBeamParams"),
        Some(true),
    )?;
    number(tractor, "recoilInterval", 0.125)?;
    let body = proof::at(tractor, "/kind/value")?;
    for (field, value) in [
        ("minForce", 0.0),
        ("maxForce", 42.0),
        ("minDistance", 0.0),
        ("maxDistance", 7.0),
        ("fullStrengthDistance", 0.0),
        ("maxVolume", 0.0),
        ("maxAngle", 0.0),
    ] {
        number(body, field, value)?;
        let cell = proof::cell(body, field, "present")?;
        assert!(proof::uses(&catalog, cell)?.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }));
    }
    assert_ne!(
        proof::at(body, "/maxForce/sourceUsesRefs")?,
        proof::at(body, "/maxDistance/sourceUsesRefs")?,
    );
    let [root] = proof::rows(body, "/authored/roots")? else {
        return Err("the unprojected source field remains in the raw arena".into());
    };
    proof::present(root, "name", &json!("fixtureOnlyFlag"), Some(true))?;
    assert_eq!(proof::at(root, "/value/valueIndex")?, "0");
    proof::present(
        proof::at(body, "/authored/values/0")?,
        "value",
        &json!({"type": "boolean", "value": true}),
        Some(true),
    )?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

fn catalog(filter: Option<&Filter>) -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        gear_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn owner<'a>(
    catalog: &'a Value,
    table: &str,
    row: usize,
    guid: u128,
    index: u32,
) -> TestResult<&'a Value> {
    let owner = proof::at(catalog, &format!("/configurations/{table}/rows/{row}"))?;
    proof::present(owner, "ownerGuid", &json!(common::guid(guid)), Some(true))?;
    proof::present(owner, "ownerRecordIndex", &json!(index), Some(true))?;
    proof::present(owner, "componentOrdinal", &json!(1), Some(true))?;
    Ok(owner)
}

fn only_additional_field(owner: &Value, name: &str) -> TestResult {
    let [field] = proof::rows(owner, "/additionalFields/roots")? else {
        return Err("exactly one unprojected original field".into());
    };
    proof::present(field, "name", &json!(name), Some(true))?;
    Ok(())
}

fn number(owner: &Value, field: &str, value: f32) -> TestResult {
    let finite: Value = serde_json::from_str(&serde_json::to_string(&value)?)?;
    proof::present(
        owner,
        field,
        &json!({"rawBits": format!("{:08x}", value.to_bits()), "finite": finite}),
        Some(true),
    )?;
    Ok(())
}
