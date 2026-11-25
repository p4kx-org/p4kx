// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{HEALTH_ONLY, database_with_health, item};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const DEVICE: &str = "EntityComponentTriggerableDevicesParams";
const PRIMEABLE: &str = "SPrimeableComponentParams";
const TRIGGER: &str = "STriggerableDevicesTriggerParams";
const TIMER: &str = "STriggerableDevicesTriggerTimerParams";
const IMPACT: &str = "STriggerableDevicesTriggerImpactParams";
const LASER_TRIP: &str = "STriggerableDevicesTriggerLaserTripParams";
const BEHAVIOR: &str = "STriggerableDevicesBehaviorParams";
const EXPLOSION_BEHAVIOR: &str = "STriggerableDevicesBehaviorExplosionParams";
const FUTURE: &str = "FixtureAdditionalTrigger";

#[test]
fn grenade_components_preserve_physical_origins_and_original_device_shapes() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database(), origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        validator
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            proof::at(&catalog, "/exportHealth/status")?,
            "complete",
            "all authored fixture fields export without projection loss"
        );
        let [device, primeable] = proof::rows(&catalog, "/configurations/explosives/rows")? else {
            return Err("equal Item owner with two physical explosive components".into());
        };
        let device = configuration(device, 1, DEVICE, "triggerableDevice", encrypted)?;
        let primeable = configuration(primeable, 2, PRIMEABLE, "primeable", encrypted)?;
        let trigger_list = proof::cell(device, "triggers", "present")?;
        let [timer, impact, laser_trip, null, future] = proof::rows(trigger_list, "/value")? else {
            return Err("all original trigger slots in physical order".into());
        };
        assert_eq!(
            null["state"], "null",
            "null keeps its original position and state"
        );
        proof::present(device, "aiTriggers", &json!([]), Some(encrypted))?;
        assert_eq!(
            proof::at(device, "/deathBehavior/state")?,
            "null",
            "the device stores a separate null death behavior"
        );
        check_triggers([timer, impact, laser_trip, future], encrypted)?;
        proof::present(primeable, "primeOnThrow", &json!(false), Some(encrypted))?;
        proof::present(
            primeable,
            "primeableAnimatedStates",
            &json!([]),
            Some(encrypted),
        )?;
        assert_eq!(
            proof::at(primeable, "/primedState/state")?,
            "null",
            "a null weak state has its own sourced cell"
        );
    }
    Ok(())
}

fn check_triggers(slots: [&Value; 4], encrypted: bool) -> TestResult {
    let [timer, impact, laser_trip, future] = slots;
    let timer = proof::at(timer, "/value")?;
    assert_eq!(timer["type"], "timer", "the original timer subtype");
    assert_eq!(
        proof::at(timer, "/sourceType/value")?,
        TIMER,
        "the exact original timer type name"
    );
    proof::present(
        timer,
        "duration",
        &json!({"rawBits": "3fc00000", "finite": 1.5}),
        Some(encrypted),
    )?;
    let behavior = proof::at(timer, "/behavior/value")?;
    assert_eq!(
        behavior["type"], "explosion",
        "the timer owns the original typed behavior"
    );
    let damage = proof::at(behavior, "/explosionParams/value/damage/value/value")?;
    proof::present(
        damage,
        "physical",
        &json!({"rawBits": "c0000000", "finite": -2.0}),
        Some(encrypted),
    )?;
    let impact = proof::at(impact, "/value")?;
    assert_eq!(impact["type"], "impact", "the original impact subtype");
    proof::present(
        impact,
        "ignoreWaterCollision",
        &json!(false),
        Some(encrypted),
    )?;
    let laser_trip = proof::at(laser_trip, "/value")?;
    assert_eq!(
        laser_trip["type"], "laserTrip",
        "the original laser trip subtype"
    );
    proof::present(
        proof::at(laser_trip, "/value")?,
        "laserRadius",
        &json!({"rawBits": "00000000", "finite": 0.0}),
        Some(encrypted),
    )?;
    assert_eq!(
        proof::at(future, "/value/type")?,
        "other",
        "readable future trigger still has a typed descriptor"
    );
    let [extra] = proof::rows(future, "/value/additionalFields/roots")? else {
        return Err("original future member is retained".into());
    };
    proof::present(extra, "name", &json!("fixtureExtra"), Some(encrypted))?;
    Ok(())
}

fn configuration<'row>(
    row: &'row Value,
    slot: u32,
    source_type: &str,
    kind: &str,
    encrypted: bool,
) -> TestResult<&'row Value> {
    proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
    proof::present(row, "sourceType", &json!(source_type), Some(encrypted))?;
    let configuration = proof::cell(row, "configuration", "present")?;
    assert_eq!(
        configuration["containsEncryptedSource"], encrypted,
        "physical configuration selector encryption follows the DataForge origin"
    );
    assert_eq!(
        proof::at(configuration, "/value/type")?,
        kind,
        "configuration type follows the physical component"
    );
    proof::at(configuration, "/value/value")
}

fn database() -> DataForgeBuilder {
    let database = database_with_health(TestStruct::new("FixtureHealth"))
        .structure(
            TestStruct::new(DEVICE)
                .parent("DataForgeComponentParams")
                .array("triggers", pointer(TRIGGER))
                .array("aiTriggers", pointer(TRIGGER))
                .field("deathBehavior", pointer(BEHAVIOR)),
        )
        .structure(
            TestStruct::new(PRIMEABLE)
                .parent("DataForgeComponentParams")
                .field("primeOnThrow", TestType::Bool)
                .field(
                    "primedState",
                    TestType::WeakPointer("FixturePrimeState".to_owned()),
                )
                .array(
                    "primeableAnimatedStates",
                    class("FixtureAnimatedPrimeState"),
                ),
        )
        .structure(TestStruct::new("FixturePrimeState"))
        .structure(TestStruct::new("FixtureAnimatedPrimeState"));
    behavior_schema(trigger_schema(database)).record(item(
        HEALTH_ONLY,
        "fixture_device_component",
        "Misc",
        vec![
            node(device()),
            node(
                TestInstance::new(PRIMEABLE)
                    .value("primeOnThrow", TestValue::Bool(false))
                    .value("primedState", TestValue::Null)
                    .value("primeableAnimatedStates", TestValue::Array(Vec::new())),
            ),
        ],
    ))
}

fn trigger_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new(TRIGGER).field("behavior", pointer(BEHAVIOR)))
        .structure(
            TestStruct::new(TIMER)
                .parent(TRIGGER)
                .field("duration", TestType::Single),
        )
        .structure(
            TestStruct::new(IMPACT)
                .parent(TRIGGER)
                .field("ignoreWaterCollision", TestType::Bool),
        )
        .structure(
            TestStruct::new(LASER_TRIP)
                .parent(TRIGGER)
                .field("laserRadius", TestType::Single),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(TRIGGER)
                .field("fixtureExtra", TestType::UInt32),
        )
}

fn behavior_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new(BEHAVIOR))
        .structure(
            TestStruct::new(EXPLOSION_BEHAVIOR)
                .parent(BEHAVIOR)
                .field("explosionParams", class("ExplosionParams")),
        )
        .structure(TestStruct::new("ExplosionParams").field("damage", pointer("DamageBase")))
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single),
        )
}

fn device() -> TestInstance {
    TestInstance::new(DEVICE)
        .value(
            "triggers",
            TestValue::Array(vec![
                node(
                    TestInstance::new(TIMER)
                        .value("duration", TestValue::Single(1.5))
                        .value("behavior", node(blast())),
                ),
                node(
                    TestInstance::new(IMPACT).value("ignoreWaterCollision", TestValue::Bool(false)),
                ),
                node(TestInstance::new(LASER_TRIP).value("laserRadius", TestValue::Single(0.0))),
                TestValue::Null,
                node(TestInstance::new(FUTURE).value("fixtureExtra", TestValue::UInt32(17))),
            ]),
        )
        .value("aiTriggers", TestValue::Array(Vec::new()))
        .value("deathBehavior", TestValue::Null)
}

fn blast() -> TestInstance {
    TestInstance::new(EXPLOSION_BEHAVIOR).value(
        "explosionParams",
        node(TestInstance::new("ExplosionParams").value(
            "damage",
            node(TestInstance::new("DamageInfo").value("DamagePhysical", TestValue::Single(-2.0))),
        )),
    )
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
