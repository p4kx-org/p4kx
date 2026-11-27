// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestStruct, TestType, TestValue,
    entity_schema,
};

pub(super) const MISSING_TAG: u128 = 0x61f0;

const EMP: &str = "SCItemEMPParams";
const MISSILE: &str = "SCItemMissileParams";
const ORDNANCE: &str = "SExplosiveOrdnanceParams";
const RECORD: &str = "ExplosionRecord";
const PARAMS: &str = "ExplosionParams";

pub(super) fn database() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(
            component(EMP)
                .field("distortionDamage", TestType::Single)
                .field("ChargingTag", TestType::ReferenceTo("Tag".to_owned()))
                .field("ChargedTag", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            component(MISSILE)
                .field("requiresLauncher", TestType::Bool)
                .field(
                    "shakeParams",
                    TestType::StrongPointer("SShakeParams".to_owned()),
                ),
        )
        .structure(component(ORDNANCE))
        .structure(TestStruct::new("SShakeParams"))
        .structure(
            TestStruct::new(RECORD).field("explosionParams", TestType::Class(PARAMS.to_owned())),
        )
        .structure(TestStruct::new(PARAMS).field("pressure", TestType::Single))
        .record(
            TestEntity::new(
                1,
                "FixtureOrdnance",
                "libs/foundry/records/entities/scitem/ships/fixture_ordnance.xml",
            )
            .attach_type("Misc")
            .component(emp())
            .component(TestValue::Struct(
                TestInstance::new(MISSILE)
                    .value("requiresLauncher", TestValue::Bool(true))
                    .value("shakeParams", TestValue::Null),
            ))
            .component(TestValue::Struct(TestInstance::new(ORDNANCE)))
            .record(),
        )
        .record(
            TestEntity::new(2, "UnadmittedOrdnance", "fixture/unadmitted_ordnance.xml")
                .attach_type("Misc")
                .component(emp())
                .record(),
        )
        .record(definition(4, 1.5))
        .record(definition(3, -0.0))
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn emp() -> TestValue {
    TestValue::Struct(
        TestInstance::new(EMP)
            .value("distortionDamage", TestValue::Single(120.0))
            .value("ChargingTag", TestValue::Reference(MISSING_TAG))
            .value("ChargedTag", TestValue::Null),
    )
}

fn definition(guid: u128, pressure: f32) -> TestRecord {
    TestRecord::new(
        guid,
        RECORD,
        format!("ExplosionFixture{guid}"),
        format!("fixture/explosion_{guid}.xml"),
    )
    .value(
        "explosionParams",
        TestValue::Struct(TestInstance::new(PARAMS).value("pressure", TestValue::Single(pressure))),
    )
}
