// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::Value as RecordValue;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{COMPONENT, FlightController};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

const OWNER: &str = "EntityClassDefinition";
const COMPONENT_BASE: &str = "FixtureComponent";
const INTERACTION: &str = "FixtureInteraction";
const EFFECT: &str = "FixtureEffect";
const COLLISION: &str = "FixtureCollision";
const RECALL: &str = "ShipRecallParams";

#[test]
fn flight_controller_preserves_original_link_storage_and_field_states() -> TestResult {
    project(&fixture().build(), |row| {
        assert_cell(row, "/ownerRecordIndex", &json!(0))?;
        assert_cell(row, "/componentOrdinal", &json!(0))?;
        assert_cell(row, "/sourceType", &json!(COMPONENT))?;
        assert_cell(
            row,
            "/fuelWarningDisplayTime",
            &json!({"finite": 0.0, "rawBits": "00000000"}),
        )?;
        assert_cell(row, "/hoverAnimPlayInLandingMode", &json!(false))?;
        assert_eq!(
            at(row, "/disableESPInteraction/state")?,
            "null",
            "declared weak pointer"
        );
        assert_eq!(
            at(row, "/enableESPInteraction/state")?,
            "absent",
            "undeclared weak pointer"
        );
        assert_eq!(
            at(row, "/hudParamsOverride/state")?,
            "null",
            "declared null Record reference"
        );
        assert_eq!(
            at(row, "/vehicleHudParamsOverride/state")?,
            "absent",
            "independent HUD reference"
        );
        assert_eq!(
            at(row, "/PowerOnEffectTag/value/status")?,
            "present",
            "physical effect reference"
        );
        assert_eq!(
            at(row, "/PowerOffEffectTag/state")?,
            "null",
            "separate effect reference"
        );
        assert_eq!(
            at(row, "/additionalFields/roots/0/name/value")?,
            "fixtureUnknown",
            "owner-local raw data"
        );
        Ok(())
    })
}

#[test]
fn flight_controller_keeps_collision_and_recall_facts_independent() -> TestResult {
    project(&fixture().build(), |row| {
        assert_cell(
            row,
            "/collisionDetection/value/collisionWarnSpeed",
            &json!({"finite": 3.0, "rawBits": "40400000"}),
        )?;
        assert_cell(
            row,
            "/collisionDetection/value/collisionDangerCloseWarnTime",
            &json!({"finite": null, "rawBits": "7fc01234"}),
        )?;
        assert_cell(row, "/ShipRecall/value/sourceType", &json!(RECALL))?;
        assert_cell(
            row,
            "/ShipRecall/value/MinimumRecallDistance",
            &json!({"finite": 9.0, "rawBits": "41100000"}),
        )?;
        assert_eq!(
            at(row, "/ShipRecall/value/AIModuleTag/value/status")?,
            "present",
            "recall tag is a Record link"
        );
        assert_eq!(
            at(row, "/ShipRecall/value/ObstructionDetectionRange/state")?,
            "absent",
            "omitted recall field stays absent"
        );
        Ok(())
    })
}

fn fixture() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(TestStruct::new(OWNER).array(
            "Components",
            TestType::StrongPointer(COMPONENT_BASE.to_owned()),
        ))
        .structure(controller_schema())
        .structure(TestStruct::new(INTERACTION))
        .structure(TestStruct::new(EFFECT))
        .structure(
            TestStruct::new(COLLISION)
                .field("collisionWarnSpeed", TestType::Single)
                .field("collisionDangerCloseWarnTime", TestType::Single),
        )
        .structure(
            TestStruct::new(RECALL)
                .field("AIModuleTag", TestType::ReferenceTo(EFFECT.to_owned()))
                .field("MinimumRecallDistance", TestType::Single),
        )
        .record(
            TestRecord::new(1, OWNER, "FixtureVehicle", "fixtures/vehicle.xml")
                .value("Components", TestValue::Array(vec![controller()])),
        )
        .record(TestRecord::new(2, EFFECT, "FixtureTag", "fixtures/tag.xml"))
}

fn controller_schema() -> TestStruct {
    TestStruct::new(COMPONENT)
        .parent(COMPONENT_BASE)
        .field("fuelWarningDisplayTime", TestType::Single)
        .field("hoverAnimPlayInLandingMode", TestType::Bool)
        .field(
            "disableESPInteraction",
            TestType::WeakPointer(INTERACTION.to_owned()),
        )
        .field(
            "hudParamsOverride",
            TestType::ReferenceTo(EFFECT.to_owned()),
        )
        .field("PowerOnEffectTag", TestType::ReferenceTo(EFFECT.to_owned()))
        .field(
            "PowerOffEffectTag",
            TestType::ReferenceTo(EFFECT.to_owned()),
        )
        .field("collisionDetection", TestType::Class(COLLISION.to_owned()))
        .field("ShipRecall", TestType::StrongPointer(RECALL.to_owned()))
        .field("fixtureUnknown", TestType::Bool)
}

fn controller() -> TestValue {
    TestValue::Struct(
        TestInstance::new(COMPONENT)
            .value("fuelWarningDisplayTime", TestValue::Single(0.0))
            .value("hoverAnimPlayInLandingMode", TestValue::Bool(false))
            .value("disableESPInteraction", TestValue::Null)
            .value("hudParamsOverride", TestValue::Null)
            .value("PowerOnEffectTag", TestValue::Reference(2))
            .value("PowerOffEffectTag", TestValue::Null)
            .value(
                "collisionDetection",
                TestValue::Struct(
                    TestInstance::new(COLLISION)
                        .value("collisionWarnSpeed", TestValue::Single(3.0))
                        .value(
                            "collisionDangerCloseWarnTime",
                            TestValue::Single(f32::from_bits(0x7fc0_1234)),
                        ),
                ),
            )
            .value(
                "ShipRecall",
                TestValue::Struct(
                    TestInstance::new(RECALL)
                        .value("AIModuleTag", TestValue::Reference(2))
                        .value("MinimumRecallDistance", TestValue::Single(9.0)),
                ),
            )
            .value("fixtureUnknown", TestValue::Bool(true)),
    )
}

fn project(bytes: &[u8], verify: impl FnOnce(&Value) -> TestResult) -> TestResult {
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
        let [component] = components.as_ref() else {
            return Err("fixture component cardinality".into());
        };
        let component = originals.fact(component)?;
        let owner = ComponentOwner::read(&tree, &context, &originals, component)?;
        let fields = Observed::configuration(&originals, input, &capture, &numbers);
        let mut notes = Vec::new();
        let row = FlightController::read(owner, fields.at_owner(component), &mut notes)?;
        assert!(notes.is_empty(), "fixture source types are accepted");
        verify(&serde_json::to_value(row)?)
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
