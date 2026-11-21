// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid};
use crate::health_origin_assertions as proof;

#[path = "security_manifest_fixture.rs"]
mod fixture;

const ROWS: &str = "/definitions/securityNetworkManifests/rows";

#[test]
fn manifest_retains_the_original_security_graph_and_field_origins() -> TestResult {
    let (_folder, input) = archive::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&input, &archive::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let [row] = proof::rows(&catalog, ROWS)? else {
        return Err("one physical manifest row expected".into());
    };
    proof::present(row, "guid", &json!(guid(fixture::MANIFEST)), Some(true))?;
    proof::present(row, "isNeutralTerritory", &json!(true), Some(true))?;
    proof::present(row, "teleportPlayerOut", &json!(false), Some(true))?;
    verify_protocol(at(row, "/defaultProtocol/value")?)?;
    let room = at(row, "/roomProtocols/value/0/value")?;
    verify_protocol(at(room, "/protocol/value")?)?;
    let tags = at(room, "/roomIdentifiers/value/tags/value")?;
    verify_reference(at(tags, "/0/value")?, fixture::TAG, "present")?;
    assert_eq!(
        at(tags, "/1/state")?,
        "null",
        "the null room Tag slot stays in order"
    );
    verify_hostility(at(row, "/hostilityRules/value/0/value")?)?;
    verify_variables(row)?;
    assert!(
        at(row, "/additionalFields")?
            .to_string()
            .contains("fixtureExtra"),
        "unmapped member retained"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    proof::closure(&catalog)
}

#[test]
fn manifest_reference_to_a_filtered_target_preserves_its_physical_guid() -> TestResult {
    let (_folder, input) = archive::open(&fixture::database(), DcbOrigin::Plain)?;
    let filter = Filter::new("selected/*");
    let mut options = archive::options();
    options.filter = Some(&filter);
    let catalog: Value = serde_json::from_slice(build_catalog(&input, &options)?.json())?;
    let [row] = proof::rows(&catalog, ROWS)? else {
        return Err("one selected manifest row expected".into());
    };
    let protocol = at(row, "/defaultProtocol/value")?;
    let owner = at(protocol, "/ownerFaction/value")?;
    assert_eq!(
        at(owner, "/guid/value")?,
        &json!(guid(fixture::OWNER)),
        "physical Faction GUID survives exclusion"
    );
    proof::present(row, "isNeutralTerritory", &json!(true), Some(false))?;
    assert_eq!(
        at(&catalog, "/completeness/securityNetworkManifests/status")?,
        "partial",
        "a path filter makes the manifest view partial"
    );
    assert!(
        proof::rows(&catalog, "/exportHealth/unresolvedReferences")?
            .iter()
            .any(|link| {
                link.pointer("/targetGuid/value") == Some(&json!(guid(fixture::OWNER)))
                    && link.get("kind").and_then(Value::as_str) == Some("securityNetworkManifests")
                    && link.get("status").and_then(Value::as_str) == Some("excludedByFilter")
            }),
        "physical target exclusion has an explicit receipt"
    );
    Ok(())
}

fn verify_protocol(protocol: &Value) -> TestResult {
    let with = at(protocol, "/permissionsWithClearance/value")?;
    let without = at(protocol, "/permissionsWithoutClearance/value")?;
    proof::present(with, "access", &json!(fixture::GRANTED), Some(true))?;
    proof::present(without, "access", &json!(fixture::DENIED), Some(true))?;
    proof::present(without, "trespass", &json!(fixture::GRANTED), Some(true))?;
    verify_reference(
        at(protocol, "/ownerFaction/value")?,
        fixture::OWNER,
        "present",
    )?;
    let tokens = at(protocol, "/acceptedClearance/value/tokens/value")?;
    verify_reference(at(tokens, "/0/value")?, fixture::TOKEN, "present")?;
    assert_eq!(
        at(tokens, "/1/state")?,
        "null",
        "the null clearance token slot stays in order"
    );
    verify_reference(at(tokens, "/2/value")?, fixture::MISSING, "dangling")?;
    Ok(())
}

fn verify_hostility(rules: &Value) -> TestResult {
    for (name, value) in [
        ("useReputationHostility", true),
        ("usePersonalHostility", false),
        ("useFactionHostility", true),
        ("policesLawfulTrespass", false),
        ("hostileToCriminals", true),
        ("hostileToStolenVehicles", false),
    ] {
        proof::present(rules, name, &json!(value), Some(true))?;
    }
    let legacy = at(rules, "/factionOverride/value")?;
    verify_reference(legacy, fixture::LEGACY, "present")?;
    assert_eq!(
        at(legacy, "/targetType/value")?,
        "Faction_LEGACY",
        "the legacy link keeps its own target type"
    );
    let current = at(rules, "/factionToOverride/value")?;
    verify_reference(current, fixture::OWNER, "present")?;
    assert_eq!(
        at(current, "/targetType/value")?,
        "Faction",
        "the current link keeps its own target type"
    );
    Ok(())
}

fn verify_variables(row: &Value) -> TestResult {
    let boolean = at(row, "/variables/value/0/value/value/value")?;
    let integer = at(row, "/variables/value/1/value/value/value")?;
    assert_eq!(
        at(boolean, "/kind")?,
        "bool",
        "the Boolean subtype has its own discriminator"
    );
    assert_eq!(
        at(integer, "/kind")?,
        "int",
        "the signed integer subtype has its own discriminator"
    );
    proof::present(boolean, "value", &json!(true), Some(true))?;
    let declared_int = proof::present(integer, "value", &json!(-7), Some(true))?;
    assert_eq!(
        at(row, "/variables/value/2/value/value/state")?,
        "null",
        "a null value pointer remains a null slot"
    );
    let group = at(row, "/variableEffects/value/0/value")?;
    let trigger = at(group, "/variableValue/value/value/value")?;
    let trigger_int = proof::present(trigger, "value", &json!(-7), Some(true))?;
    assert_ne!(
        at(declared_int, "/sourceUsesRefs")?,
        at(trigger_int, "/sourceUsesRefs")?,
        "equal integer values at different authored locations have distinct receipts"
    );
    verify_effects(at(group, "/effects/value")?)
}

fn verify_effects(effects: &Value) -> TestResult {
    assert_eq!(
        at(effects, "/0/value/kind")?,
        "setInteractionState",
        "the interaction-state effect is typed"
    );
    assert_eq!(
        at(effects, "/0/value/state/value/target/type")?,
        "record",
        "the interaction state remains a weak Record link"
    );
    assert_eq!(
        at(effects, "/1/value/kind")?,
        "setHostilityRules",
        "the hostility override effect is typed"
    );
    verify_hostility(at(effects, "/1/value/rules/value")?)?;
    assert_eq!(
        at(effects, "/2/value/kind")?,
        "authored",
        "a new effect uses the complete authored fallback"
    );
    assert!(
        at(effects, "/2/value/additionalFields")?
            .to_string()
            .contains("payload"),
        "unknown owned subtype is lossless"
    );
    assert_eq!(
        at(effects, "/3/state")?,
        "null",
        "the null effect slot stays in order"
    );
    Ok(())
}

fn verify_reference(row: &Value, expected: u128, status: &str) -> TestResult {
    proof::present(row, "guid", &json!(guid(expected)), Some(true))?;
    assert_eq!(
        at(row, "/status")?,
        status,
        "the link keeps its original availability"
    );
    Ok(())
}
