// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Filter;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const DEVICE: &str = "SCItemJumpDriveParams";
const EFFECT: &str = "FixtureJumpEffects";
const TRAVEL: &str = "FixtureJumpTravel";
const RANGE: &str = "FixtureJumpRange";
const TUNING: &str = "JumpDriveFlightParams";
const TUNNEL_FORCES: &str = "JumpTunnelForcesParams";
const STATE: &str = "ItemResourceState";
const OWNER_PATH: &str = "libs/foundry/records/entities/scitem/ships/fixture_jump.xml";
const TUNING_GUID: u128 = 0x10E7;
const ROWS: &str = "/configurations/jumpDrives/rows";

#[test]
fn jump_drive_keeps_device_values_phase_and_link_origins() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database(false), origin)?;
        let filter = Filter::new("*/fixture_jump");
        let mut options = common::options();
        options.filter = Some(&filter);
        let built = build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let row = proof::rows(&catalog, ROWS)?.first().ok_or("jump drive")?;
        proof::present(row, "sourceType", &json!(DEVICE), Some(encrypted))?;
        proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
        proof::present(row, "componentOrdinal", &json!(0), Some(encrypted))?;
        number(&catalog, row, "alignmentRate", 0.0, encrypted)?;
        proof::cell(row, "tuningRate", "absent")?;
        proof::cell(row, "tunnelForces", "null")?;
        proof::cell(row, "idleState", "null")?;
        let tuning = proof::at(proof::cell(row, "flightTuning", "present")?, "/value")?;
        proof::present(
            tuning,
            "guid",
            &json!(common::guid(TUNING_GUID)),
            Some(encrypted),
        )?;
        proof::present(tuning, "targetType", &json!(TUNING), Some(encrypted))?;
        verify_tuning_definition(&catalog, tuning, encrypted)?;
        let effects = proof::at(proof::cell(row, "effects", "present")?, "/value")?;
        proof::cell(effects, "exit", "absent")?;
        let travel = proof::at(proof::cell(effects, "travel", "present")?, "/value")?;
        let range = proof::at(
            proof::cell(travel, "tunnelResponseDistanceAheadRange", "present")?,
            "/value",
        )?;
        number(&catalog, range, "minimum", -1.0, encrypted)?;
        number(&catalog, range, "maximum", f32::MAX, encrypted)?;
        assert_eq!(
            proof::rows(effects, "/additionalFields/roots")?.len(),
            1,
            "unmodeled fields stay available"
        );
    }
    Ok(())
}

#[test]
fn jump_drive_reports_unsupported_phase_storage() -> TestResult {
    let (_folder, archive) = common::open(&database(true), DcbOrigin::Plain)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "ordinary publication rejects a wrong phase declaration"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::rows(&catalog, ROWS)?.first().ok_or("jump drive")?;
    let effects = proof::at(proof::cell(row, "effects", "present")?, "/value")?;
    proof::cell(effects, "travel", "invalid")?;
    assert_eq!(
        proof::rows(effects, "/additionalFields/roots")?.len(),
        2,
        "diagnostics retain the original malformed value"
    );
    Ok(())
}

fn verify_tuning_definition(catalog: &Value, tuning: &Value, encrypted: bool) -> TestResult {
    let rows = proof::rows(catalog, "/configurations/jumpDriveFlightTuning/rows")?;
    assert_eq!(
        rows.len(),
        1,
        "only the linked definition survives the device filter"
    );
    let definition = rows.first().ok_or("linked flight-tuning Record")?;
    let source = proof::at(definition, "/source")?;
    let target_index = proof::at(tuning, "/targetRecordIndex/value")?;
    proof::present(source, "recordIndex", target_index, Some(encrypted))?;
    let roots = proof::rows(definition, "/fields/roots")?;
    assert_eq!(
        roots.len(),
        1,
        "the selected tuning Record keeps its original fields"
    );
    proof::present(
        roots.first().ok_or("original tuning root")?,
        "name",
        &json!("respoolTime"),
        Some(encrypted),
    )?;
    let nodes = proof::rows(definition, "/fields/values")?;
    assert_eq!(
        nodes.len(),
        1,
        "the root field points to its own original value"
    );
    let value = proof::present(
        nodes.first().ok_or("original tuning node")?,
        "value",
        &json!({"type":"number","value":{"type":"f32","finite":0.0,"rawBits":"00000000"}}),
        Some(encrypted),
    )?;
    assert!(
        !proof::uses(catalog, value)?.is_empty(),
        "the definition value has an original receipt"
    );
    Ok(())
}

fn number(catalog: &Value, row: &Value, field: &str, value: f32, encrypted: bool) -> TestResult {
    let finite: Value = serde_json::from_slice(&serde_json::to_vec(&value)?)?;
    let cell = proof::present(
        row,
        field,
        &json!({"type":"f32","finite":finite,"rawBits":format!("{:08x}", value.to_bits())}),
        Some(encrypted),
    )?;
    assert!(
        !proof::uses(catalog, cell)?.is_empty(),
        "the device value has an original receipt"
    );
    Ok(())
}

fn database(bad_travel: bool) -> DataForgeBuilder {
    let (travel_type, travel_value) = if bad_travel {
        (TestType::UInt32, TestValue::UInt32(41))
    } else {
        (class(TRAVEL), fixture_travel())
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new(DEVICE)
                .parent("DataForgeComponentParams")
                .field("alignmentRate", TestType::Single)
                .field("flightTuning", TestType::ReferenceTo(TUNING.to_owned()))
                .field(
                    "tunnelForces",
                    TestType::ReferenceTo(TUNNEL_FORCES.to_owned()),
                )
                .field("idleState", TestType::WeakPointer(STATE.to_owned()))
                .field("jumpDriveEffectParams", class(EFFECT)),
        )
        .structure(
            TestStruct::new(EFFECT)
                .field("travelParams", travel_type)
                .field("fixtureUnknownFlag", TestType::Bool),
        )
        .structure(TestStruct::new(TRAVEL).field("tunnelResponseDistanceAheadRange", class(RANGE)))
        .structure(
            TestStruct::new(RANGE)
                .field("minimum", TestType::Single)
                .field("maximum", TestType::Single),
        )
        .structure(TestStruct::new(TUNING).field("respoolTime", TestType::Single))
        .structure(TestStruct::new(TUNNEL_FORCES))
        .structure(TestStruct::new(STATE))
        .record(fixture_device(travel_value))
        .record(
            TestRecord::new(
                TUNING_GUID,
                TUNING,
                "FixtureFlightTuning.test",
                "fixture/jump-tuning.xml",
            )
            .value("respoolTime", TestValue::Single(0.0)),
        )
        .record(
            TestRecord::new(
                TUNING_GUID + 1,
                TUNING,
                "FixtureFlightTuning.other",
                "fixture/jump-other.xml",
            )
            .value("respoolTime", TestValue::Single(1.0)),
        )
}

fn fixture_device(travel: TestValue) -> TestRecord {
    let effects = TestInstance::new(EFFECT)
        .value("travelParams", travel)
        .value("fixtureUnknownFlag", TestValue::Bool(false));
    let component = TestInstance::new(DEVICE)
        .value("alignmentRate", TestValue::Single(0.0))
        .value("flightTuning", TestValue::Reference(TUNING_GUID))
        .value("tunnelForces", TestValue::Null)
        .value("idleState", TestValue::Null)
        .value("jumpDriveEffectParams", TestValue::Struct(effects));
    TestRecord::new(
        0x1077,
        "EntityClassDefinition",
        "EntityClassDefinition.fixture_jump",
        OWNER_PATH,
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(component)]),
    )
}

fn fixture_travel() -> TestValue {
    let range = TestInstance::new(RANGE)
        .value("minimum", TestValue::Single(-1.0))
        .value("maximum", TestValue::Single(f32::MAX));
    TestValue::Struct(
        TestInstance::new(TRAVEL)
            .value("tunnelResponseDistanceAheadRange", TestValue::Struct(range)),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
