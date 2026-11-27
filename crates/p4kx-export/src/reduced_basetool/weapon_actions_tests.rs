// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::Value;

use super::collect;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const WEAPON: &str = "SCItemWeaponComponentParams";
const ACTION: &str = "SWeaponActionParams";
const RAPID: &str = "SWeaponActionFireRapidParams";
const BURST: &str = "SWeaponActionFireBurstParams";
const BEAM: &str = "SWeaponActionFireBeamParams";
const TRACTOR: &str = "SWeaponActionFireTractorBeamParams";
const TOGGLE: &str = "SWeaponActionToggleParams";
const PARALLEL: &str = "SWeaponActionParallelParams";
const AIM: &str = "SWeaponActionAimSimpleParams";
const DAMAGE: &str = "DamageBase";
const INFO: &str = "DamageInfo";

#[test]
fn weapon_actions_keep_authored_rates_beams_and_tractor_ranges_at_their_physical_slots()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let table = serde_json::to_value(collect(&session.parsed(None)?, scratch.path(), None)?)?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("weapon rows")?;
    let [primary, secondary] = rows.as_slice() else {
        return Err("both physical weapon components".into());
    };
    for (row, position) in [(primary, 0), (secondary, 1)] {
        assert_eq!(
            row.pointer("/ownerRecordIndex/value"),
            Some(&Value::from(0))
        );
        assert_eq!(
            row.pointer("/componentOrdinal/value"),
            Some(&Value::from(position))
        );
        assert_eq!(
            row.pointer("/ownerGuid/containsEncryptedSource"),
            Some(&Value::Bool(true))
        );
    }
    verify_fire_rates(primary, secondary)?;
    verify_tractor(primary)?;
    verify_beam_and_aim(primary)
}

fn verify_fire_rates(primary: &Value, secondary: &Value) -> TestResult {
    for (row, node_slot, kind, rate) in [
        (primary, "/fireActions/value/0/value", "rapid", 540),
        (secondary, "/fireActions/value/0/value", "burst", 60),
    ] {
        let node = action(row, row, node_slot)?;
        assert_eq!(
            node.pointer("/kind/type").and_then(Value::as_str),
            Some(kind)
        );
        assert_eq!(
            node.pointer("/kind/value/fireRate/value"),
            Some(&Value::from(rate))
        );
        assert_eq!(
            node.pointer("/kind/value/fireRateUnit")
                .and_then(Value::as_str),
            Some("dischargesPerMinute")
        );
        assert!(node.pointer("/kind/value/launchParams").is_none());
        assert!(
            node.pointer("/sourceValueOrdinal/value")
                .is_some_and(Value::is_string)
        );
    }
    Ok(())
}

fn verify_tractor(primary: &Value) -> TestResult {
    let toggle = action(primary, primary, "/fireActions/value/1/value")?;
    assert_eq!(
        toggle.pointer("/kind/type").and_then(Value::as_str),
        Some("toggle")
    );
    let tractor = action(primary, toggle, "/kind/value/weaponAction/value")?;
    assert_eq!(
        tractor.pointer("/kind/type").and_then(Value::as_str),
        Some("tractorBeam")
    );
    for (field, number) in [
        ("minForce", 100.0),
        ("maxForce", 5000.0),
        ("minDistance", 1.0),
        ("maxDistance", 100.0),
        ("fullStrengthDistance", 33.0),
        ("maxVolume", 12.0),
    ] {
        assert_eq!(
            tractor
                .pointer(&format!("/kind/value/{field}/value/finite"))
                .and_then(Value::as_f64),
            Some(number)
        );
    }
    assert!(tractor.pointer("/kind/value/maxAngle").is_none());
    assert!(tractor.pointer("/kind/value/authored").is_none());
    Ok(())
}

fn verify_beam_and_aim(primary: &Value) -> TestResult {
    let parallel = action(primary, primary, "/fireActions/value/2/value")?;
    assert_eq!(
        parallel.pointer("/kind/type").and_then(Value::as_str),
        Some("parallel")
    );
    assert_eq!(
        parallel
            .pointer("/kind/value/weaponActions/value/1/state")
            .and_then(Value::as_str),
        Some("null")
    );
    let beam = action(primary, parallel, "/kind/value/weaponActions/value/0/value")?;
    assert_eq!(
        beam.pointer("/kind/type").and_then(Value::as_str),
        Some("beam")
    );
    assert_eq!(
        beam.pointer("/kind/value/hitType/value")
            .and_then(Value::as_str),
        Some("Laser")
    );
    assert_eq!(
        beam.pointer("/kind/value/damagePerSecond/value/value/energy/value/finite")
            .and_then(Value::as_f64),
        Some(12.5)
    );
    assert_eq!(
        beam.pointer("/kind/value/damagePerSecond/value/value/distortion/state")
            .and_then(Value::as_str),
        Some("absent")
    );
    assert_eq!(
        action(primary, primary, "/aimAction/value")?
            .pointer("/kind/type")
            .and_then(Value::as_str),
        Some("aim")
    );
    Ok(())
}

fn action<'a>(row: &'a Value, parent: &Value, slot: &str) -> TestResult<&'a Value> {
    let target = parent
        .pointer(slot)
        .and_then(Value::as_u64)
        .ok_or("local action link")?;
    row.get("actionNodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            usize::try_from(target)
                .ok()
                .and_then(|index| nodes.get(index))
        })
        .ok_or_else(|| "the local node must resolve within its component".into())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(WEAPON)
                .parent(COMPONENT)
                .array("fireActions", pointer(ACTION))
                .field("aimAction", pointer(ACTION)),
        )
        .structure(TestStruct::new(ACTION))
        .structure(
            TestStruct::new(RAPID)
                .parent(ACTION)
                .field("fireRate", TestType::Int32),
        )
        .structure(
            TestStruct::new(BURST)
                .parent(ACTION)
                .field("fireRate", TestType::Int32),
        )
        .structure(TestStruct::new(AIM).parent(ACTION))
        .structure(
            TestStruct::new(TOGGLE)
                .parent(ACTION)
                .field("weaponAction", pointer(ACTION)),
        )
        .structure(
            TestStruct::new(PARALLEL)
                .parent(ACTION)
                .array("weaponActions", pointer(ACTION)),
        )
        .structure(
            TestStruct::new(BEAM)
                .parent(ACTION)
                .field("hitType", TestType::String)
                .field("damagePerSecond", pointer(DAMAGE)),
        )
        .structure(TestStruct::new(DAMAGE))
        .structure(
            TestStruct::new(INFO)
                .parent(DAMAGE)
                .field("DamageEnergy", TestType::Single),
        )
        .structure(tractor_schema())
        .record(
            TestRecord::new(
                0x1177,
                ENTITY,
                "EntityClassDefinition.FixtureWeapon",
                "libs/foundry/records/entities/scitem/fixture/weapon.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![primary(), weapon(vec![rate(BURST, 60)])]),
            ),
        )
        .build()
}

fn primary() -> TestValue {
    let beam = TestValue::Struct(
        TestInstance::new(BEAM)
            .value("hitType", TestValue::text("Laser"))
            .value(
                "damagePerSecond",
                TestValue::Struct(
                    TestInstance::new(INFO).value("DamageEnergy", TestValue::Single(12.5)),
                ),
            ),
    );
    let parallel = TestValue::Struct(TestInstance::new(PARALLEL).value(
        "weaponActions",
        TestValue::Array(vec![beam, TestValue::Null]),
    ));
    let toggle = TestValue::Struct(TestInstance::new(TOGGLE).value("weaponAction", tractor()));
    TestValue::Struct(
        TestInstance::new(WEAPON)
            .value(
                "fireActions",
                TestValue::Array(vec![rate(RAPID, 540), toggle, parallel]),
            )
            .value("aimAction", TestValue::Struct(TestInstance::new(AIM))),
    )
}

fn tractor_schema() -> TestStruct {
    tractor_values().iter().fold(
        TestStruct::new(TRACTOR).parent(ACTION),
        |schema, (key, _)| schema.field(*key, TestType::Single),
    )
}

fn tractor() -> TestValue {
    TestValue::Struct(
        tractor_values()
            .iter()
            .fold(TestInstance::new(TRACTOR), |value, (key, number)| {
                value.value(*key, TestValue::Single(*number))
            }),
    )
}

fn tractor_values() -> [(&'static str, f32); 7] {
    [
        ("minForce", 100.0),
        ("maxForce", 5000.0),
        ("minDistance", 1.0),
        ("maxDistance", 100.0),
        ("fullStrengthDistance", 33.0),
        ("maxVolume", 12.0),
        ("maxAngle", 5.0),
    ]
}

fn rate(kind: &str, rate: i32) -> TestValue {
    TestValue::Struct(TestInstance::new(kind).value("fireRate", TestValue::Int32(rate)))
}

fn weapon(actions: Vec<TestValue>) -> TestValue {
    TestValue::Struct(TestInstance::new(WEAPON).value("fireActions", TestValue::Array(actions)))
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
