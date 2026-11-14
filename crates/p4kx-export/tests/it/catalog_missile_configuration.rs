// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_missile_configuration_fixture.rs"]
mod fixture;

const MISSILES: &str = "/configurations/missileConfigurations/rows";
const EXPLOSIVES: &str = "/configurations/explosives/rows";

#[test]
fn missile_configuration_preserves_distinct_phases_origins_and_shared_ordnance() -> TestResult {
    let database = fixture::database(fixture::Layout::Exact);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    assert_eq!(
        built.json(),
        build_catalog(&archive, &common::options())?.json()
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the actual missile catalog matches schema v3"
    );
    let [first, second] = proof::rows(&catalog, MISSILES)? else {
        return Err("two original missile components".into());
    };
    verify_ordnance(&catalog, [first, second])?;
    let guidance = proof::at(first, "/guidance/value")?;
    verify_guidance(&catalog, guidance)?;
    let targeting = proof::at(first, "/targeting/value")?;
    proof::present(
        targeting,
        "trackingSignalType",
        &json!("fixtureFutureSignal"),
        Some(true),
    )?;
    proof::present(targeting, "allowDumbFiring", &json!(false), Some(true))?;
    proof::present(targeting, "lockRangeMin", &float(-0.0), Some(true))?;
    proof::present(targeting, "maxTimesCanMiss", &json!(0), Some(true))?;
    proof::cell(targeting, "lockRangeMax", "absent")?;
    verify_extra_reference(targeting, fixture::LINK, "present")?;
    verify_extra_reference(
        proof::at(second, "/targeting/value")?,
        fixture::MISSING,
        "unreadable",
    )?;
    let shake = proof::at(first, "/shake/value")?;
    proof::present(shake, "frequency", &float(3.0), Some(true))?;
    proof::present(
        proof::at(shake, "/offsetAngle/value")?,
        "sourceType",
        &json!("Ang3"),
        Some(true),
    )?;
    proof::present(
        proof::at(shake, "/offsetPosition/value")?,
        "sourceType",
        &json!("Vec3"),
        Some(true),
    )?;
    proof::cell(second, "shake", "null")?;
    Ok(())
}

fn verify_ordnance(catalog: &Value, missiles: [&Value; 2]) -> TestResult {
    let explosive = proof::rows(catalog, EXPLOSIVES)?;
    for (index, (row, ordnance)) in missiles.iter().copied().zip(explosive).enumerate() {
        for field in ["ownerGuid", "ownerRecordIndex", "componentOrdinal"] {
            assert_eq!(
                row.get(field),
                ordnance.get(field),
                "shared physical slot identity"
            );
        }
        proof::present(
            row,
            "ownerGuid",
            &json!(common::guid(fixture::OWNER)),
            Some(true),
        )?;
        proof::present(row, "componentOrdinal", &json!(index + 1), Some(true))?;
        let ordnance = proof::at(ordnance, "/configuration/value/value")?;
        proof::present(ordnance, "requiresLauncher", &json!(false), Some(true))?;
        proof::present(ordnance, "enableLifetime", &json!(true), Some(true))?;
        proof::present(ordnance, "projectileProximity", &float(0.0), Some(true))?;
        let emissions = proof::at(ordnance, "/emissionsParams/value/0/value")?;
        proof::present(emissions, "active", &json!(false), Some(true))?;
        proof::present(emissions, "minValue", &float(7.0), Some(true))?;
        let roots = proof::rows(row, "/additionalFields/roots")?;
        let [root] = roots else {
            return Err("only genuinely unmapped root field".into());
        };
        assert_eq!(
            proof::at(root, "/name/value")?,
            "fixtureUnknownRoot",
            "typed missile and shared ordnance fields are retained once"
        );
    }
    Ok(())
}

fn verify_guidance(catalog: &Value, guidance: &Value) -> TestResult {
    proof::present(guidance, "linearSpeed", &float(512.0), Some(true))?;
    for (field, raw) in [
        ("pidIntegralTerm", 11.0),
        ("pidDerivativeTerm", 12.0),
        ("pidProportionalTerm", 13.0),
    ] {
        proof::present(guidance, field, &float(raw), Some(true))?;
    }
    let mut distinct_origin = std::collections::BTreeSet::new();
    for (phase, seed) in [
        ("boostPhase", 10.0),
        ("interceptPhase", 20.0),
        ("terminalPhase", 30.0),
    ] {
        let phase = proof::at(proof::cell(guidance, phase, "present")?, "/value")?;
        proof::present(phase, "angularSpeed", &float(seed), Some(true))?;
        proof::present(phase, "maxRotationAccel", &float(seed + 4.0), Some(true))?;
        proof::present(phase, "pidAggression", &float(seed + 5.0), Some(true))?;
        for (field, direction) in [
            ("maxLinearAccelerationPositive", 1.0),
            ("maxLinearAccelerationNegative", -1.0),
        ] {
            let axes = proof::at(proof::cell(phase, field, "present")?, "/value")?;
            for (axis, offset) in [("x", 1.0), ("y", 2.0), ("z", 3.0)] {
                let cell =
                    proof::present(axes, axis, &float(direction * (seed + offset)), Some(true))?;
                let uses = proof::uses(catalog, cell)?;
                let use_location = uses
                    .iter()
                    .find(|usage| {
                        usage.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                            && usage.pointer("/role").and_then(Value::as_str) == Some("payload")
                    })
                    .ok_or("original axis payload source")?;
                assert!(
                    distinct_origin.insert(
                        proof::at(use_location, "/location/indices/1")?
                            .as_str()
                            .ok_or("original field position")?
                    ),
                    "each phase, sign and axis has its own original source occurrence"
                );
            }
        }
    }
    Ok(())
}

fn verify_extra_reference(targeting: &Value, guid: u128, state: &str) -> TestResult {
    let root = proof::at(targeting, "/additionalFields/roots/0")?;
    assert_eq!(
        proof::at(root, "/name/value")?,
        "fixtureLaunchHint",
        "the original additional Reference stays rooted on targeting"
    );
    let node = proof::at(targeting, "/additionalFields/values/0")?;
    let cell = proof::cell(node, "value", state)?;
    let actual = if state == "present" {
        proof::at(cell, "/value/value/guid/value")?
    } else {
        proof::at(node, "/originalError/guid/value")?
    };
    assert_eq!(
        actual,
        &json!(common::guid(guid)),
        "generic arena preserves stored Reference"
    );
    Ok(())
}

#[test]
fn missile_configuration_diagnoses_wrong_class_or_enum_with_original_occurrences() -> TestResult {
    for (layout, field, state) in [
        (
            fixture::Layout::WrongTargetingClass,
            "targeting",
            "unsupported",
        ),
        (
            fixture::Layout::WrongSignalEnum,
            "trackingSignalType",
            "unsupported",
        ),
    ] {
        let (_folder, archive) = common::open(&fixture::database(layout), DcbOrigin::Plain)?;
        assert!(
            matches!(
                build_catalog(&archive, &common::options()),
                Err(CatalogError::IncompleteExport(_))
            ),
            "normal publication refuses a changed missile declaration"
        );
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let row = proof::rows(&catalog, MISSILES)?
            .first()
            .ok_or("original missile stays present")?;
        let owner = if field == "targeting" {
            row
        } else {
            proof::at(row, "/targeting/value")?
        };
        proof::cell(owner, field, state)?;
        let roots = proof::rows(owner, "/additionalFields/roots")?;
        let raw_field = if field == "targeting" {
            "targetingParams"
        } else {
            "trackingSignalType"
        };
        assert!(
            roots
                .iter()
                .any(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(raw_field)),
            "a failed typed projection retains the readable original field"
        );
    }
    Ok(())
}

fn float(value: f32) -> Value {
    json!({"rawBits": format!("{:08x}", value.to_bits()), "finite": value})
}
