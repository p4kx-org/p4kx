// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const WRAPPER: &str = "SEntityPhysicsControllerParams";
const BASE: &str = "SEntityBasePhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const RIGID_FIELD: &str = "fixtureRigidField";

#[test]
fn rigid_ammo_keeps_original_fields_and_completes_normal_catalog() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [row] = proof::rows(&catalog, "/definitions/ammoDefinitions/rows")? else {
        return Err("one original Ammo definition".into());
    };
    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "complete",
        "the rigid controller's raw Double satisfies numeric coverage"
    );
    let physics = proof::at(row, "/physicsControllerParams/value/physType/value")?;
    proof::present(
        physics,
        "mass",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    check_rigid_members(&catalog, proof::at(physics, "/kind")?)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

fn check_rigid_members(catalog: &Value, kind: &Value) -> TestResult {
    assert_eq!(
        proof::at(kind, "/type")?,
        "rigid",
        "the actual concrete original controller selects its branch"
    );
    let [field] = proof::rows(kind, "/additionalFields/roots")? else {
        return Err("one raw field without duplicating the typed mass".into());
    };
    let [value] = proof::rows(kind, "/additionalFields/values")? else {
        return Err("one exact original rigid value".into());
    };
    proof::present(field, "name", &json!(RIGID_FIELD), Some(true))?;
    assert_eq!(
        proof::at(field, "/value/valueIndex")?,
        "0",
        "the original field points to its local arena value"
    );
    let payload = proof::present(
        value,
        "value",
        &json!({"type":"number","value":{"type":"f64","rawBits":"7ff8000000001234","finite":null}}),
        Some(true),
    )?;
    assert!(
        proof::uses(catalog, payload)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices") == Some(&json!(["0", "3"]))
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the raw Double cites its own original value occurrence"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("AmmoParams").field(
            "physicsControllerParams",
            TestType::StrongPointer(WRAPPER.to_owned()),
        ))
        .structure(
            TestStruct::new(WRAPPER).field("PhysType", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE).field("Mass", TestType::Single))
        .structure(
            TestStruct::new(RIGID)
                .parent(BASE)
                .field(RIGID_FIELD, TestType::Double),
        )
        .record(
            TestRecord::new(
                1,
                "AmmoParams",
                "AmmoParams.RigidFixture",
                "libs/foundry/records/ammoparams/rigid_fixture.xml",
            )
            .value(
                "physicsControllerParams",
                TestValue::Struct(
                    TestInstance::new(WRAPPER).value(
                        "PhysType",
                        TestValue::Struct(
                            TestInstance::new(RIGID)
                                .value("Mass", TestValue::Single(-0.0))
                                .value(
                                    RIGID_FIELD,
                                    TestValue::Double(f64::from_bits(0x7ff8_0000_0000_1234)),
                                ),
                        ),
                    ),
                ),
            ),
        )
}
