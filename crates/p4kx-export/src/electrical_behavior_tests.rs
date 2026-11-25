// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, OpenOptions, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ElectricalBehavior;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::weather::{
    LightningTargetMode, LightningTargetPayload, with_electrical_behaviors,
};
use crate::{CatalogOptions, build_catalog};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const OWNER: &str = "ElectricalBehavior";
const LIGHTNING: &str = "LightningBehavior";
const EFFECT: &str = "LightningBehavior_Effect";
const TARGET: &str = "LightningTargetMode";
const INTERNAL: &str = "LightningTargetMode_InternalPlanet";
const OTHER: &str = "FixtureExternalMode";
const RANGE: &str = "Range";

#[test]
fn electrical_behaviors_share_catalog_and_selected_sdk_projection() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(&path, OpenOptions::new().key(Some(test_key())))?;
    let projected = with_electrical_behaviors(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            let [owner] = snapshot.rows() else {
                return Err("expected the selected physical Record".into());
            };
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "null slots and unfamiliar declared types are fully represented"
            );
            verify_owner(owner)?;
            Ok::<_, Box<dyn std::error::Error>>(serde_json::to_value(snapshot.rows())?)
        },
    )??;
    let catalog = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let json: Value = serde_json::from_slice(catalog.json())?;
    let rows = json
        .pointer("/configurations/electricalBehaviors/rows")
        .and_then(Value::as_array)
        .ok_or("electrical behaviour table")?;
    let [row] = rows.as_slice() else {
        return Err("only owners are selected by the table".into());
    };
    let sdk = projected
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or("selected SDK row")?;
    assert_eq!(
        row, sdk,
        "SDK and catalog share the same typed row and origin claims"
    );
    assert_eq!(
        row.pointer(
            "/lightning/value/effects/value/0/value/targetModes/value/0/value/payload/value/kind"
        ),
        Some(&json!("internalPlanet")),
        "v3 carries the original type as a stable classification"
    );
    Ok(())
}

fn verify_owner(owner: &ElectricalBehavior<'_>) -> TestResult {
    assert_eq!(
        owner.source().record_index().value(),
        Some(&0),
        "the enclosing source is the physical Record"
    );
    let lightning = owner
        .lightning()
        .value()
        .ok_or("original lightning Class")?;
    let [effect] = lightning
        .effects()
        .value()
        .ok_or("original effect array")?
        .as_slice()
    else {
        return Err("one original effect is expected".into());
    };
    let effect = effect.value().ok_or("original effect slot")?;
    let [internal, null, other] = effect
        .target_modes()
        .value()
        .ok_or("original target-mode array")?
        .as_slice()
    else {
        return Err("three original target slots are expected".into());
    };
    let internal = internal.value().ok_or("original internal target slot")?;
    verify_internal(internal)?;
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "a null pointer keeps its physical slot"
    );
    let other = other.value().ok_or("original unfamiliar target slot")?;
    assert!(
        matches!(other.payload().value(), Some(LightningTargetPayload::Other)),
        "an unfamiliar derived type remains present"
    );
    assert_eq!(
        other.source_type().value(),
        Some(&OTHER),
        "the exact unfamiliar type is available"
    );
    let [field] = other.additional_fields().roots() else {
        return Err("the unfamiliar target retains its original field".into());
    };
    assert_eq!(
        field.name().value(),
        Some(&"fixtureThreshold"),
        "uninterpreted fields remain addressable"
    );
    Ok(())
}

fn verify_internal(internal: &LightningTargetMode<'_>) -> TestResult {
    let Some(LightningTargetPayload::InternalPlanet(parameters)) = internal.payload().value()
    else {
        return Err("the first pointer selects the concrete internal target mode".into());
    };
    assert_eq!(
        internal.enable().value(),
        Some(&true),
        "the parent declares the inherited flag"
    );
    assert_eq!(
        internal.payload().origin().contains_encrypted_source(),
        Some(true),
        "the classification is derived from the encrypted source type"
    );
    assert_eq!(
        parameters
            .intensity
            .value()
            .map(crate::resource_float::ResourceFloat32::raw_bits),
        Some(0x8000_0000),
        "signed zero remains the exact authored Single"
    );
    let distance = parameters
        .distance_range
        .value()
        .ok_or("authored distance range")?;
    assert_eq!(
        (
            distance
                .minimum()
                .value()
                .and_then(crate::resource_float::ResourceFloat32::finite),
            distance
                .maximum()
                .value()
                .and_then(crate::resource_float::ResourceFloat32::finite),
        ),
        (Some(12.0), Some(6.0)),
        "distance endpoints retain source order without normalisation"
    );
    assert_eq!(
        distance.minimum().origin().contains_encrypted_source(),
        Some(true),
        "each range endpoint has an independent original provenance marker"
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    let effects = TestValue::Array(vec![TestValue::Struct(
        TestInstance::new(EFFECT).value("targetModes", target_modes()),
    )]);
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(OWNER).field("lightning", TestType::Class(LIGHTNING.to_owned())))
        .structure(TestStruct::new(LIGHTNING).array("effects", TestType::Class(EFFECT.to_owned())))
        .structure(
            TestStruct::new(EFFECT)
                .array("targetModes", TestType::StrongPointer(TARGET.to_owned())),
        )
        .structure(TestStruct::new(TARGET).field("enable", TestType::Bool))
        .structure(
            TestStruct::new(INTERNAL)
                .parent(TARGET)
                .field("minimumInterval", TestType::Single)
                .field("referenceInterval", TestType::Single)
                .field("length", TestType::Class(RANGE.to_owned()))
                .field("scaleMultiplier", TestType::Single)
                .field("intensity", TestType::Single)
                .field("distanceRange", TestType::Class(RANGE.to_owned())),
        )
        .structure(
            TestStruct::new(RANGE)
                .field("minimum", TestType::Single)
                .field("maximum", TestType::Single),
        )
        .structure(
            TestStruct::new(OTHER)
                .parent(TARGET)
                .field("fixtureThreshold", TestType::UInt32),
        )
        .record(
            TestRecord::new(
                1,
                OWNER,
                "ElectricalBehavior.Fixture",
                "fixture/weather.xml",
            )
            .value(
                "lightning",
                TestValue::Struct(TestInstance::new(LIGHTNING).value("effects", effects)),
            ),
        )
        .build()
}

fn target_modes() -> TestValue {
    TestValue::Array(vec![
        TestValue::Struct(
            TestInstance::new(INTERNAL)
                .value("enable", TestValue::Bool(true))
                .value("minimumInterval", TestValue::Single(2.0))
                .value("referenceInterval", TestValue::Single(4.0))
                .value("length", range(1.0, 2.0))
                .value("scaleMultiplier", TestValue::Single(3.0))
                .value("intensity", TestValue::Single(-0.0))
                .value("distanceRange", range(12.0, 6.0)),
        ),
        TestValue::Null,
        TestValue::Struct(
            TestInstance::new(OTHER).value("fixtureThreshold", TestValue::UInt32(21)),
        ),
    ])
}

fn range(minimum: f32, maximum: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(RANGE)
            .value("minimum", TestValue::Single(minimum))
            .value("maximum", TestValue::Single(maximum)),
    )
}
