// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue, entity_schema,
};

pub(super) const OWNER: u128 = 0x7618;
pub(super) const DURABILITY: &str = "/configurations/durability";
pub(super) const DEGRADATION: &str = "/configurations/degradation";
pub(super) const DISTORTION: &str = "/configurations/distortion";

const HEALTH_TYPE: &str = "SHealthComponentParams";
const DEGRADATION_TYPE: &str = "SDegradationParams";
const DISTORTION_TYPE: &str = "SDistortionParams";

pub(super) fn database() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(component_type(HEALTH_TYPE, "Health", TestType::Single))
        .structure(component_type(
            DEGRADATION_TYPE,
            "StopDegradingIfDestroyed",
            TestType::Bool,
        ))
        .structure(component_type(DISTORTION_TYPE, "Maximum", TestType::Single))
        .record(
            item("Alpha")
                .component(health(100.0))
                .component(TestValue::Struct(
                    TestInstance::new(DEGRADATION_TYPE)
                        .value("StopDegradingIfDestroyed", TestValue::Bool(true)),
                ))
                .component(health(f32::from_bits(0x7fc0_1234)))
                .record(),
        )
        .record(
            item("Beta")
                .component(TestValue::Struct(
                    TestInstance::new(DISTORTION_TYPE).value("Maximum", TestValue::Single(80.0)),
                ))
                .component(health(40.0))
                .record(),
        )
}

fn component_type(name: &str, field: &str, field_type: TestType) -> TestStruct {
    TestStruct::new(name)
        .parent("DataForgeComponentParams")
        .field(field, field_type)
}

fn item(name: &str) -> TestEntity {
    TestEntity::new(
        OWNER,
        format!("FixtureDurability{name}"),
        format!("libs/foundry/records/entities/scitem/ships/fixture_durability_{name}.xml"),
    )
    .attach_type("Misc")
}

fn health(value: f32) -> TestValue {
    TestValue::Struct(TestInstance::new(HEALTH_TYPE).value("Health", TestValue::Single(value)))
}
