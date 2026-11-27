// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestStruct, TestType, TestValue,
    entity_schema,
};

const ENTITY: u128 = 0x1177_8001;
const DEFINITION: u128 = 0x1177_8002;
const DANGLING: u128 = 0x1177_8003;
const WRONG_TYPE: u128 = 0x1177_8004;
const HEALTH: &str = "SHealthComponentParams";
const SUIT: &str = "SCItemSuitArmorParams";
const MACRO: &str = "DamageResistanceMacro";
const BODY: &str = "DamageResistance";
const ENTRY: &str = "DamageResistanceEntry";
const FOREIGN: &str = "FixtureUnrelatedMacro";
const COMPONENT_BASE: &str = "DataForgeComponentParams";

pub(super) fn complete() -> DataForgeBuilder {
    schema(TestType::Single)
        .record(resistance("primary", 0.625, channel(0.75)))
        .record(
            item("Alpha")
                .component(health(TestValue::Single(100.0)))
                .component(suit(DEFINITION))
                .component(health(TestValue::Single(f32::NAN)))
                .component(suit(0))
                .record(),
        )
        .record(resistance("alias", 0.25, TestValue::Null))
        .record(
            item("Beta")
                .component(suit(DANGLING))
                .component(health(TestValue::Single(-0.0)))
                .component(suit(WRONG_TYPE))
                .record(),
        )
        .record(TestRecord::new(
            WRONG_TYPE,
            FOREIGN,
            "FixtureSomethingElse",
            "fixtures/suit/other.xml",
        ))
}

pub(super) fn invalid_health() -> DataForgeBuilder {
    schema(TestType::Bool).record(
        item("BadHealth")
            .component(health(TestValue::Bool(true)))
            .record(),
    )
}

fn schema(health_type: TestType) -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new(HEALTH)
                .parent(COMPONENT_BASE)
                .field("Health", health_type)
                .field("DamageCap", TestType::Single),
        )
        .structure(
            TestStruct::new(SUIT)
                .parent(COMPONENT_BASE)
                .field("damageResistance", TestType::ReferenceTo(MACRO.to_owned()))
                .field("integrityMilestoneToBreak", TestType::Single),
        )
        .structure(
            TestStruct::new(MACRO).field("damageResistance", TestType::Class(BODY.to_owned())),
        )
        .structure(
            TestStruct::new(BODY)
                .field("IgnoreMeleeDamage", TestType::Bool)
                .field("PhysicalResistance", TestType::Class(ENTRY.to_owned()))
                .field(
                    "EnergyResistance",
                    TestType::StrongPointer(ENTRY.to_owned()),
                )
                .field("DistortionResistance", TestType::Class(ENTRY.to_owned()))
                .field("ThermalResistance", TestType::Class(ENTRY.to_owned()))
                .field("BiochemicalResistance", TestType::Class(ENTRY.to_owned())),
        )
        .structure(
            TestStruct::new(ENTRY)
                .field("Multiplier", TestType::Single)
                .field("Threshold", TestType::Single)
                .field("DamageCap", TestType::Single)
                .field("DamageCapLimit", TestType::Single),
        )
        .structure(TestStruct::new(FOREIGN))
}

fn item(name: &str) -> TestEntity {
    TestEntity::new(
        ENTITY,
        format!("Fixture{name}"),
        format!("libs/foundry/records/entities/scitem/gear/durability_{name}.xml"),
    )
    .attach_type("Misc")
}

fn health(value: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HEALTH)
            .value("Health", value)
            .value("DamageCap", TestValue::Single(75.0)),
    )
}

fn suit(target: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new(SUIT)
            .value("damageResistance", TestValue::Reference(target))
            .value("integrityMilestoneToBreak", TestValue::Single(0.5)),
    )
}

fn resistance(name: &str, multiplier: f32, energy: TestValue) -> TestRecord {
    let body = TestInstance::new(BODY)
        .value("IgnoreMeleeDamage", TestValue::Bool(true))
        .value("PhysicalResistance", channel(multiplier))
        .value("EnergyResistance", energy)
        .value("DistortionResistance", channel(0.25))
        .value("ThermalResistance", channel(f32::INFINITY))
        .value("BiochemicalResistance", channel(-0.5));
    TestRecord::new(
        DEFINITION,
        MACRO,
        format!("{MACRO}.{name}"),
        format!("fixtures/suit/{name}.xml"),
    )
    .value("damageResistance", TestValue::Struct(body))
}

fn channel(multiplier: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(ENTRY)
            .value("Multiplier", TestValue::Single(multiplier))
            .value("Threshold", TestValue::Single(2.0))
            .value("DamageCap", TestValue::Single(80.0))
            .value("DamageCapLimit", TestValue::Single(25.0)),
    )
}
