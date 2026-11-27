// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::Value as Original;
use serde_json::{Value, json};

use super::{AuthoredProjectile, Observed};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "authored_projectile_fixture.rs"]
mod fixture;

#[path = "projectile_class_array_tests.rs"]
mod class_arrays;

#[test]
fn ammo_projectile_keeps_inherited_and_bullet_behavior_in_the_same_occurrence() -> TestResult {
    with_actual(0, |actual| {
        let root = at(&actual.json, "/value")?;
        assert_eq!(at(root, "/kind/type")?, "bullet", "physical Bullet branch");
        check_detonation(root)?;
        let bullet = at(root, "/kind/value")?;
        assert_eq!(
            at(bullet, "/impactRadius/value")?,
            &json!({"rawBits":"80000000","finite":-0.0}),
            "stored signed zero"
        );
        assert_eq!(
            at(bullet, "/keepAliveOnZeroDamage/value")?,
            false,
            "stored false"
        );
        check_bullet_behavior(bullet)?;
        check_numbers(&actual.numbers)?;
        let radius = at(bullet, "/impactRadius")?;
        assert_eq!(
            at(radius, "/containsEncryptedSource")?,
            false,
            "the actual fixture DCB is plain"
        );
        check_exact_use(radius, &actual.inventory, &actual.impact_radius_location)?;
        assert_eq!(
            actual.notes, 0,
            "the complete typed Bullet fixture is readable"
        );
        Ok(())
    })
}

#[test]
fn ammo_projectile_keeps_tachyon_range_and_explicit_source_states() -> TestResult {
    with_actual(1, |actual| {
        let root = at(&actual.json, "/value")?;
        assert_eq!(
            at(root, "/kind/type")?,
            "tachyon",
            "physical Tachyon branch"
        );
        let tachyon = at(root, "/kind/value")?;
        assert_eq!(
            at(tachyon, "/fullDamageRange/value")?,
            &json!({"rawBits":"00000000","finite":0.0}),
            "original full range keeps zero"
        );
        assert_eq!(
            at(tachyon, "/zeroDamageRange/value")?,
            &json!({"rawBits":"40000000","finite":2.0}),
            "original zero-damage range is independent"
        );
        assert_eq!(
            at(root, "/detonationParams/state")?,
            "null",
            "actual null pointer"
        );
        assert_eq!(
            at(root, "/proximityTriggerParams/state")?,
            "null",
            "actual null trigger"
        );
        assert_eq!(actual.notes, 0, "the Tachyon fixture is readable");
        Ok(())
    })?;
    with_actual(2, |actual| {
        assert_eq!(
            at(&actual.json, "/state")?,
            "unsupported",
            "unrecognized concrete source"
        );
        assert_eq!(
            actual.notes, 1,
            "the original unsupported type has one field diagnosis"
        );
        Ok(())
    })?;
    with_actual(3, |actual| {
        assert_eq!(
            at(&actual.json, "/state")?,
            "null",
            "explicitly null projectile"
        );
        assert_eq!(actual.notes, 0, "a null pointer is readable");
        Ok(())
    })
}

#[test]
fn ammo_projectile_keeps_visual_beam_and_countermeasure_sources() -> TestResult {
    with_actual(0, |actual| {
        let bullet = at(&actual.json, "/value/kind/value")?;
        assert_eq!(
            at(bullet, "/visualParams/value/material/value/path/value")?,
            "fixture/normal.mtl",
            "original primary material path"
        );
        assert_eq!(
            at(
                bullet,
                "/alternateVisualParams/value/material/value/path/value"
            )?,
            "fixture/alternate.mtl",
            "original alternate material path"
        );
        Ok(())
    })?;
    with_actual(1, |actual| {
        let beams = at(&actual.json, "/value/kind/value/beamEffects/value")?
            .as_array()
            .ok_or("original beam slots")?;
        let [first, last] = beams.as_slice() else {
            return Err("two original beam slots".into());
        };
        assert_eq!(
            at(first, "/value/helper/value")?,
            "fixture-beam",
            "original first beam"
        );
        assert_eq!(
            at(last, "/value/helper/value")?,
            "fixture-beam",
            "original duplicate beam"
        );
        assert_ne!(
            at(first, "/sourceUsesRefs")?,
            at(last, "/sourceUsesRefs")?,
            "identical beams retain separate original occurrence receipts"
        );
        Ok(())
    })?;
    with_actual(5, |actual| check_countermeasure(&actual.json, "chaff"))?;
    with_actual(6, |actual| check_countermeasure(&actual.json, "flare"))
}

fn check_countermeasure(document: &Value, kind: &str) -> TestResult {
    let projectile = at(document, "/value/kind")?;
    assert_eq!(
        at(projectile, "/type")?,
        "countermeasure",
        "actual source variant"
    );
    let settings = at(projectile, "/value/typeParams/value")?;
    assert_eq!(
        at(settings, "/type")?,
        kind,
        "original countermeasure subtype"
    );
    assert_eq!(
        at(settings, "/value/startInfrared/value/rawBits")?,
        "3f800000",
        "stored signature endpoint"
    );
    let trigger = at(
        projectile,
        "/value/trailStartAudioTrigger/value/audioTrigger/value",
    )?;
    assert_eq!(
        trigger, "fixture-trigger",
        "the shared audio resource reader preserves the path"
    );
    Ok(())
}

fn check_detonation(root: &Value) -> TestResult {
    let detonation = at(root, "/detonationParams/value")?;
    assert_eq!(
        at(detonation, "/explodeOnFinalImpact/value")?,
        true,
        "authored trigger flag"
    );
    let explosion = at(detonation, "/explosionParams/value")?;
    assert_eq!(
        at(explosion, "/minRadius/value/rawBits")?,
        "3f800000",
        "shared explosion reader projects the exact value"
    );
    assert_eq!(
        at(explosion, "/damage/value/type")?,
        "info",
        "shared authored damage reader retains the inline subtype"
    );
    assert_eq!(
        at(root, "/proximityTriggerParams/value/radius/value/rawBits")?,
        "40000000",
        "the actual inherited proximity radius"
    );
    Ok(())
}

fn check_bullet_behavior(bullet: &Value) -> TestResult {
    let expected = [
        ("/damage/value/type", json!("info")),
        (
            "/damageDropParams/value/damageDropMinDistance/value/type",
            json!("base"),
        ),
        (
            "/damageDropParams/value/damageDropPerMeter/value/type",
            json!("parameters"),
        ),
        (
            "/damageDropParams/value/damageDropMinDamage/value/type",
            json!("info"),
        ),
        (
            "/impulseFalloffParams/value/maxFalloff/value/rawBits",
            json!("00000000"),
        ),
        (
            "/pierceabilityParams/value/damageFalloffLevel2/value/rawBits",
            json!("3f800000"),
        ),
        (
            "/penetrationParams/value/nearRadius/value/rawBits",
            json!("3f000000"),
        ),
        ("/electronParams/value/maximumJumps/value", json!(-2)),
        (
            "/additionalProjectilesParams/value/ammoRecord/value/targetRecordIndex/value",
            json!(0),
        ),
        ("/hitBehaviors/value/0/value/type", json!("damageOverTime")),
        ("/hitBehaviors/value/1/state", json!("null")),
        ("/hitBehaviors/value/2/value/type", json!("base")),
    ];
    for (path, value) in expected {
        assert_eq!(at(bullet, path)?, &value, "original behavior at {path}");
    }
    let total = at(
        bullet,
        "/damageDropParams/value/damageDropPerMeter/value/value/damageTotal",
    )?;
    assert_eq!(at(total, "/state")?, "present", "readable authored NaN");
    assert_eq!(
        at(total, "/value")?,
        &json!({"rawBits":"7fc01234","finite":null}),
        "exact original NaN bits"
    );
    Ok(())
}

fn check_numbers(numbers: &[CapturedNumber]) -> TestResult {
    let [value] = numbers else {
        return Err("one emitted nonfinite original projectile number".into());
    };
    assert_eq!(value.record_index, 0, "original physical Ammo Record");
    assert_eq!(
        value.token,
        FloatToken::Single(0x7fc0_1234),
        "exact original bits"
    );
    assert!(
        matches!(
            value.occurrence,
            NumberOccurrence::AuthoredDefinitionValue { .. }
        ),
        "uses the exact authored-definition numeric receipt family"
    );
    assert_eq!(
        value.field, "projectileParams.damageDropParams.damageDropPerMeter.damageTotal",
        "retains the exact authored field path"
    );
    Ok(())
}

fn check_exact_use(cell: &Value, inventory: &Value, location: &Value) -> TestResult {
    let refs = at(cell, "/sourceUsesRefs")?
        .as_array()
        .ok_or("emitted source refs")?;
    let uses = at(inventory, "/uses")?
        .as_array()
        .ok_or("original source inventory")?;
    assert!(
        uses.iter().any(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.get("role").and_then(Value::as_str) == Some("payload")
                && used.get("location") == Some(location)
        }),
        "the payload cites this exact admitted original field occurrence"
    );
    Ok(())
}

struct Actual {
    json: Value,
    inventory: Value,
    impact_radius_location: Value,
    numbers: Vec<CapturedNumber>,
    notes: usize,
}

fn with_actual(index: u32, check: impl FnOnce(Actual) -> TestResult) -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(index).ok_or("original Ammo Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let output = AuthoredProjectile::read_ammo(
            Observed::definition(&fields, input, &capture, &numbers),
            &mut notes,
        )?;
        let owner = fields
            .field(None, "projectileParams")?
            .map(|(_, fact)| fact)
            .filter(|fact| matches!(fact.value(), Original::Struct(_)));
        let location = if let Some(owner) = owner {
            fields
                .field(Some(owner), "impactRadius")?
                .map(|(_, fact)| fact.location(OriginalValuePart::Value))
        } else {
            None
        };
        check(Actual {
            json: serde_json::to_value(output)?,
            inventory: serde_json::to_value(scope.sources().freeze()?)?,
            impact_radius_location: serde_json::to_value(location)?,
            numbers: numbers.into_inner(),
            notes: notes.len(),
        })
    })
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing actual projectile field {path}").into())
}
