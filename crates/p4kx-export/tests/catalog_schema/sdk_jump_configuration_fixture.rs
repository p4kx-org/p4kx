// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const GLOBAL_GUID: u128 = 0x3c71;

const ENTITY: &str = "EntityClassDefinition";
const BASE: &str = "FixtureComponent";
const JUMP: &str = "SJumpPointParams";
const POINT: &str = "GlobalJumpPointParams";
const HOST: &str = "GlobalJumpTunnelHostParams";
const FORCE: &str = "JumpTunnelForcesParams";

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(JUMP)
                .parent(BASE)
                .field("requiredFuel", TestType::Single),
        )
        .structure(TestStruct::new(POINT).field("alignmentRange", TestType::Single))
        .structure(TestStruct::new(HOST).field("entityPullInAcceleration", TestType::Single))
        .structure(TestStruct::new(FORCE).field("fixtureForce", TestType::Single))
        .record(
            TestRecord::new(
                1,
                ENTITY,
                "EntityClassDefinition.JumpPoint",
                "fixture/jump/point.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Struct(TestInstance::new(BASE)),
                    TestValue::Struct(
                        TestInstance::new(JUMP).value("requiredFuel", TestValue::Single(3.25)),
                    ),
                ]),
            ),
        )
        .record(settings(GLOBAL_GUID, POINT, "alignmentRange", -0.0))
        .record(settings(GLOBAL_GUID, HOST, "entityPullInAcceleration", 0.5))
        .record(settings(2, FORCE, "fixtureForce", 8.0))
}

fn settings(guid: u128, kind: &str, field: &str, value: f32) -> TestRecord {
    TestRecord::new(guid, kind, kind, format!("fixture/jump/{kind}.xml"))
        .value(field, TestValue::Single(value))
}
