// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::Value as RecordValue;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{COMPONENT, Thruster};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

const OWNER: &str = "EntityClassDefinition";
const COMPONENT_BASE: &str = "FixtureComponent";
const ROLE: &str = "FixtureThrusterRole";
const PRIMARY: &str = "FixturePrimary";
const MANEUVER: &str = "FixtureManeuver";
const RESOURCE_UNIT: &str = "SBaseResourceUnit";
const MICRO_UNIT: &str = "SMicroResourceUnit";
const GIMBAL: &str = "FixtureGimbal";
const AXIS: &str = "FixtureGimbalAxis";
const OVERRIDE: &str = "FixtureGimbalOverride";

#[test]
fn physical_thruster_rows_keep_original_roles_capacities_and_occurrences() -> TestResult {
    let bytes = fixture().build();
    project(&bytes, |first, second| {
        assert_cell(first, "/ownerRecordIndex", &json!(0))?;
        assert_cell(first, "/componentOrdinal", &json!(0))?;
        assert_cell(second, "/componentOrdinal", &json!(1))?;
        assert_cell(first, "/sourceType", &json!(COMPONENT))?;
        assert_cell(first, "/thrusterType", &json!(PRIMARY))?;
        assert_cell(second, "/thrusterType", &json!(MANEUVER))?;
        assert_cell(
            first,
            "/thrustCapacity",
            &json!({"finite": -0.0, "rawBits":"80000000"}),
        )?;
        assert_cell(
            first,
            "/thrustCapacityNew",
            &json!({"finite": 7.5, "rawBits":"40f00000"}),
        )?;
        assert_cell(
            second,
            "/thrustCapacityNew",
            &json!({"finite": null, "rawBits":"7fc01234"}),
        )?;
        assert_eq!(
            at(first, "/onlyActiveInVTOL/state")?,
            "absent",
            "a source without a declaration remains absent"
        );
        assert_eq!(
            at(first, "/nozzleAnimation/value")?,
            "",
            "a declared empty label is present"
        );
        assert_eq!(
            at(first, "/additionalFields/roots/0/name/value")?,
            "fixtureUnknown",
            "unprojected original member survives"
        );
        assert_ne!(
            at(first, "/thrustCapacity/sourceUsesRefs")?,
            at(second, "/thrustCapacity/sourceUsesRefs")?,
            "repeated occurrence keeps separate contribution coordinates"
        );
        Ok(())
    })
}

#[test]
fn thruster_fuel_gimbal_and_backwash_keep_nested_source_shapes() -> TestResult {
    let bytes = fixture().build();
    project(&bytes, |first, second| {
        assert_eq!(
            at(first, "/fuelBurnRatePer10KNewtonRN/value/kind")?,
            "microResourceUnit",
            "concrete authored unit is retained"
        );
        assert_eq!(
            at(second, "/fuelBurnRatePer10KNewtonRN/state")?,
            "null",
            "null pointer remains distinct from a zero rate"
        );
        assert_cell(
            first,
            "/thrusterBackwashSize/value/x",
            &json!({"finite": 2.0, "rawBits":"40000000"}),
        )?;
        assert_cell(
            first,
            "/gimbal/value/pitchAxis/value/speed",
            &json!({"finite": 2.0, "rawBits":"40000000"}),
        )?;
        assert_cell(
            first,
            "/gimbal/value/yawAxis/value/speed",
            &json!({"finite": 4.0, "rawBits":"40800000"}),
        )?;
        assert_cell(
            first,
            "/gimbalInteractionStateOverrides/value/0/value/gimbalParams/value/pitchAxis/value/speed",
            &json!({"finite": 9.0, "rawBits":"41100000"}),
        )?;
        assert_eq!(
            at(
                first,
                "/gimbalInteractionStateOverrides/value/0/value/interactionState/state"
            )?,
            "null",
            "optional weak pointer is a source state"
        );
        assert_eq!(
            at(second, "/gimbalInteractionStateOverrides/value")?,
            &json!([]),
            "declared empty override array remains present"
        );
        Ok(())
    })
}

fn fixture() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(ROLE, [PRIMARY, MANEUVER])
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(TestStruct::new(OWNER).array(
            "Components",
            TestType::StrongPointer(COMPONENT_BASE.to_owned()),
        ))
        .structure(thruster_type())
        .structure(TestStruct::new(RESOURCE_UNIT))
        .structure(
            TestStruct::new(MICRO_UNIT)
                .parent(RESOURCE_UNIT)
                .field("microResourceUnits", TestType::Int32),
        )
        .structure(
            TestStruct::new("Vec3")
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field("z", TestType::Single),
        )
        .structure(TestStruct::new(AXIS).field("speed", TestType::Single))
        .structure(
            TestStruct::new(GIMBAL)
                .field("pitchAxis", TestType::Class(AXIS.to_owned()))
                .field("yawAxis", TestType::Class(AXIS.to_owned())),
        )
        .structure(TestStruct::new("FixtureInteractionState"))
        .structure(
            TestStruct::new(OVERRIDE)
                .field(
                    "interactionState",
                    TestType::WeakPointer("FixtureInteractionState".to_owned()),
                )
                .field("gimbalParams", TestType::Class(GIMBAL.to_owned())),
        )
        .record(
            TestRecord::new(1, OWNER, "FixtureVehicle", "fixtures/vehicle.xml").value(
                "Components",
                TestValue::Array(vec![
                    thruster(FixtureThruster::Main, 7.5),
                    thruster(FixtureThruster::Maneuver, f32::from_bits(0x7fc0_1234)),
                ]),
            ),
        )
}

fn thruster_type() -> TestStruct {
    TestStruct::new(COMPONENT)
        .parent(COMPONENT_BASE)
        .field("thrusterType", TestType::Enum(ROLE.to_owned()))
        .field("thrustCapacity", TestType::Single)
        .field("thrustCapacityNew", TestType::Single)
        .field("nozzleAnimation", TestType::String)
        .field("fixtureUnknown", TestType::Bool)
        .field(
            "fuelBurnRatePer10KNewtonRN",
            TestType::StrongPointer(RESOURCE_UNIT.to_owned()),
        )
        .field("thrusterBackwashSize", TestType::Class("Vec3".to_owned()))
        .field("gimbal", TestType::Class(GIMBAL.to_owned()))
        .array(
            "gimbalInteractionStateOverrides",
            TestType::Class(OVERRIDE.to_owned()),
        )
}

#[derive(Clone, Copy)]
enum FixtureThruster {
    Main,
    Maneuver,
}

fn thruster(thruster: FixtureThruster, new_capacity: f32) -> TestValue {
    let (role, fuel, overrides) = match thruster {
        FixtureThruster::Main => {
            let fuel = TestValue::Struct(
                TestInstance::new(MICRO_UNIT).value("microResourceUnits", TestValue::Int32(3)),
            );
            let override_row = TestValue::Struct(
                TestInstance::new(OVERRIDE)
                    .value("interactionState", TestValue::Null)
                    .value("gimbalParams", gimbal(9.0, 3.0)),
            );
            (PRIMARY, fuel, vec![override_row])
        }
        FixtureThruster::Maneuver => (MANEUVER, TestValue::Null, Vec::new()),
    };
    TestValue::Struct(
        TestInstance::new(COMPONENT)
            .value("thrusterType", TestValue::text(role))
            .value("thrustCapacity", TestValue::Single(-0.0))
            .value("thrustCapacityNew", TestValue::Single(new_capacity))
            .value("nozzleAnimation", TestValue::text(""))
            .value("fixtureUnknown", TestValue::Bool(true))
            .value("fuelBurnRatePer10KNewtonRN", fuel)
            .value(
                "thrusterBackwashSize",
                TestValue::Struct(TestInstance::new("Vec3").value("x", TestValue::Single(2.0))),
            )
            .value("gimbal", gimbal(2.0, 4.0))
            .value(
                "gimbalInteractionStateOverrides",
                TestValue::Array(overrides),
            ),
    )
}

fn gimbal(pitch: f32, yaw: f32) -> TestValue {
    let axis =
        |speed| TestValue::Struct(TestInstance::new(AXIS).value("speed", TestValue::Single(speed)));
    TestValue::Struct(
        TestInstance::new(GIMBAL)
            .value("pitchAxis", axis(pitch))
            .value("yawAxis", axis(yaw)),
    )
}

fn project(bytes: &[u8], verify: impl FnOnce(&Value, &Value) -> TestResult) -> TestResult {
    test_support::with_input(bytes, |forge, input| {
        let record = forge.record(0).ok_or("fixture Entity Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(admission.record(), &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let context = input.origins().record(tree.record_index());
        let numbers = RefCell::new(Vec::new());
        let Some(RecordValue::Array(components)) = tree.field("Components") else {
            return Err("fixture components".into());
        };
        let [first, second] = components.as_ref() else {
            return Err("fixture has two physical components".into());
        };
        let row = |component| -> TestResult<Value> {
            let component = originals.fact(component)?;
            let owner = ComponentOwner::read(&tree, &context, &originals, component)?;
            let fields = Observed::configuration(&originals, input, &capture, &numbers);
            let mut notes = Vec::new();
            let row = Thruster::read(owner, fields.at_owner(component), &mut notes)?;
            assert!(notes.is_empty(), "fixture source shapes are supported");
            Ok(serde_json::to_value(row)?)
        };
        verify(&row(first)?, &row(second)?)
    })
}

fn at<'row>(row: &'row Value, pointer: &str) -> TestResult<&'row Value> {
    row.pointer(pointer)
        .ok_or_else(|| format!("missing {pointer}").into())
}

fn assert_cell(row: &Value, pointer: &str, expected: &Value) -> TestResult {
    let cell = at(row, pointer)?;
    assert_eq!(at(cell, "/state")?, "present", "{pointer}");
    assert_eq!(at(cell, "/value")?, expected, "{pointer}");
    assert_eq!(at(cell, "/containsEncryptedSource")?, false, "{pointer}");
    assert_eq!(at(cell, "/sourceSetComplete")?, true, "{pointer}");
    assert!(
        at(cell, "/sourceUsesRefs")?
            .as_array()
            .is_some_and(|refs| !refs.is_empty()),
        "missing per-value source contribution at {pointer}",
    );
    Ok(())
}
