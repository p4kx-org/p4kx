// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{AuthoredAmmoInputs, Observed};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "ammo_scalar_storage_tests.rs"]
mod scalar_storage;

const BASE_CARGO_UNIT: &str = "SBaseCargoUnit";
const STANDARD_CARGO_UNIT: &str = "SStandardCargoUnit";
const CENTI_CARGO_UNIT: &str = "SCentiCargoUnit";
const MICRO_CARGO_UNIT: &str = "SMicroCargoUnit";
const WRAPPER: &str = "SEntityPhysicsControllerParams";
const BASE_PHYSICS: &str = "SEntityBasePhysicsControllerParams";
const PARTICLE: &str = "SEntityParticlePhysicsControllerParams";

#[test]
fn ammo_inputs_keeps_original_cargo_units_and_particle_storage() -> TestResult {
    let rows = emitted()?;
    let [standard, centi, micro, base] = rows.as_slice() else {
        return Err("four original Ammo source Records".into());
    };
    check_conversion(
        standard,
        "standard",
        "standardCargoUnits",
        &json!({"rawBits":"3f000000","finite":0.5}),
    )?;
    check_conversion(centi, "centi", "centiScu", &json!(-7))?;
    check_conversion(micro, "micro", "microScu", &json!(0))?;
    assert_eq!(
        at(base, "/conversionRate/value/type")?,
        "base",
        "a stored fieldless cargo base is present"
    );
    check_physics(standard)?;
    assert_eq!(
        at(standard, "/size/value")?,
        &json!(u8::MAX),
        "authored root UInt8 retains its full range"
    );
    assert_eq!(
        at(standard, "/bulletType/value")?,
        &json!(-3),
        "authored root Int32 retains its sign"
    );
    assert_eq!(
        at(standard, "/shotsPerAudioLoop/value")?,
        &json!(u32::MAX),
        "authored root UInt32 retains its full range"
    );
    Ok(())
}

fn check_conversion(value: &Value, kind: &str, field: &str, expected: &Value) -> TestResult {
    let conversion = at(value, "/conversionRate/value")?;
    assert_eq!(at(conversion, "/type")?, kind, "the actual cargo unit type");
    assert_eq!(
        at(conversion, &format!("/{field}/value"))?,
        expected,
        "the exact stored amount in its own cargo unit"
    );
    Ok(())
}

fn check_physics(value: &Value) -> TestResult {
    let physics = at(value, "/physicsControllerParams/value/physType/value")?;
    assert_eq!(
        at(physics, "/kind/type")?,
        "particle",
        "the actual owned PhysType"
    );
    assert_eq!(
        at(physics, "/mass/value")?,
        &json!({"rawBits":"80000000","finite":-0.0}),
        "inherited mass retains signed zero"
    );
    let particle = at(physics, "/kind/value")?;
    assert_eq!(
        at(particle, "/pierceability/value")?,
        &json!(u32::MAX),
        "authored particle UInt32"
    );
    assert_eq!(
        at(particle, "/disableGravity/value")?,
        false,
        "stored false on the particle subtype"
    );
    assert_eq!(
        at(particle, "/airResistance/value/rawBits")?,
        "3f800000",
        "stored raw air resistance"
    );
    Ok(())
}

fn emitted() -> TestResult<Vec<Value>> {
    let mut result = Vec::new();
    test_support::with_input(&bytes(), |forge, input| {
        for record in forge.records() {
            let tree = record.tree()?;
            let admission = RecordObservation::capture(input, &record)?;
            let scope = input.record_scope(&record, &tree)?;
            let original = OriginalFlowFields::read(&scope, None)?;
            let capture = TemplateFieldCapture::new(&original, input, &admission)?;
            let numbers = RefCell::new(Vec::new());
            let mut notes = Vec::new();
            let output = AuthoredAmmoInputs::read(
                Observed::definition(&original, input, &capture, &numbers),
                &mut notes,
            )?;
            assert!(
                notes.is_empty(),
                "the synthetic Ammo source is completely readable"
            );
            result.push(serde_json::to_value(output)?);
        }
        Ok(())
    })?;
    Ok(result)
}

fn bytes() -> Vec<u8> {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("size", TestType::UInt8)
                .field("bulletType", TestType::Int32)
                .field("shotsPerAudioLoop", TestType::UInt32)
                .field("conversionRate", pointer(BASE_CARGO_UNIT))
                .field("physicsControllerParams", pointer(WRAPPER)),
        )
        .structure(TestStruct::new(BASE_CARGO_UNIT))
        .structure(
            TestStruct::new(STANDARD_CARGO_UNIT)
                .parent(BASE_CARGO_UNIT)
                .field("standardCargoUnits", TestType::Single),
        )
        .structure(
            TestStruct::new(CENTI_CARGO_UNIT)
                .parent(BASE_CARGO_UNIT)
                .field("centiSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new(MICRO_CARGO_UNIT)
                .parent(BASE_CARGO_UNIT)
                .field("microSCU", TestType::Int32),
        );
    physics_types(builder)
        .record(ammo(
            1,
            TestInstance::new(STANDARD_CARGO_UNIT)
                .value("standardCargoUnits", TestValue::Single(0.5)),
        ))
        .record(ammo(
            2,
            TestInstance::new(CENTI_CARGO_UNIT).value("centiSCU", TestValue::Int32(-7)),
        ))
        .record(ammo(
            3,
            TestInstance::new(MICRO_CARGO_UNIT).value("microSCU", TestValue::Int32(0)),
        ))
        .record(ammo(4, TestInstance::new(BASE_CARGO_UNIT)))
        .build()
}

fn physics_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(WRAPPER).field("PhysType", pointer(BASE_PHYSICS)))
        .structure(TestStruct::new(BASE_PHYSICS).field("Mass", TestType::Single))
        .structure(
            TestStruct::new(PARTICLE)
                .parent(BASE_PHYSICS)
                .field("pierceability", TestType::UInt32)
                .field("disableGravity", TestType::Bool)
                .field("airResistance", TestType::Single),
        )
}

fn ammo(id: u128, conversion: TestInstance) -> TestRecord {
    let particle = TestInstance::new(PARTICLE)
        .value("Mass", TestValue::Single(-0.0))
        .value("pierceability", TestValue::UInt32(u32::MAX))
        .value("disableGravity", TestValue::Bool(false))
        .value("airResistance", TestValue::Single(1.0));
    TestRecord::new(
        id,
        "AmmoParams",
        format!("AmmoParams.Fixture{id}"),
        format!("fixture/ammo_{id}.xml"),
    )
    .value("size", TestValue::UInt8(u8::MAX))
    .value("bulletType", TestValue::Int32(-3))
    .value("shotsPerAudioLoop", TestValue::UInt32(u32::MAX))
    .value("conversionRate", TestValue::Struct(conversion))
    .value(
        "physicsControllerParams",
        TestValue::Struct(
            TestInstance::new(WRAPPER).value("PhysType", TestValue::Struct(particle)),
        ),
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing actual Ammo field {path}").into())
}
