// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

use super::super::{HEALTH_ONLY, item};

#[derive(Clone, Copy)]
pub(super) enum Case {
    Valid,
    BadTrigger,
    WrongStorage,
}

pub(super) fn database(case: Case) -> DataForgeBuilder {
    schema(case).record(item(
        HEALTH_ONLY,
        "fixture_explosive_components",
        "Misc",
        device_slots(case),
    ))
}

fn device_slots(case: Case) -> Vec<TestValue> {
    match case {
        Case::Valid => vec![
            mine(node(laser())),
            mine(node(proximity())),
            mine(node(TestInstance::new("SSensorMineTriggerType"))),
            mine(TestValue::Null),
            health(),
            bomb(),
            node(
                TestInstance::new("SCItemSpaceMineParams")
                    .value("emissionsParams", TestValue::Array(Vec::new())),
            ),
            node(TestInstance::new("SCItemMissileParams")),
            node(TestInstance::new("SExplosiveOrdnanceParams")),
        ],
        Case::BadTrigger => vec![
            mine(node(TestInstance::new("FixtureFutureSensorTrigger"))),
            mine(TestValue::Raw(
                [(u32::MAX - 1).to_le_bytes(), 0_u32.to_le_bytes()].concat(),
            )),
            mine(node(laser())),
        ],
        Case::WrongStorage => vec![mine(node(laser())), bomb()],
    }
}

fn schema(case: Case) -> DataForgeBuilder {
    let database = DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Misc"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            TestStruct::new("SHealthComponentParams")
                .parent("DataForgeComponentParams")
                .field("DeathExplosionParams", pointer("ExplosionBaseParams")),
        )
        .structure(TestStruct::new("GlobalResourceParticle").field("path", TestType::String))
        .structure(TestStruct::new("Vec3").field("x", TestType::Single));
    ordnance_schema(sensor_schema(database, case), case)
}

fn sensor_schema(database: DataForgeBuilder, case: Case) -> DataForgeBuilder {
    let trigger = if matches!(case, Case::WrongStorage) {
        class("SSensorMineLaserTrigger")
    } else {
        pointer("SSensorMineTriggerType")
    };
    database
        .structure(
            TestStruct::new("SSensorMineComponentParams")
                .parent("DataForgeComponentParams")
                .field("FriendlyFire", TestType::Bool)
                .field("LaserVFX", class("GlobalResourceParticle"))
                .field("TriggerType", trigger),
        )
        .structure(TestStruct::new("SSensorMineTriggerType"))
        .structure(
            trigger_type("SSensorMineLaserTrigger")
                .field("LaserLength", TestType::Single)
                .field("LaserHitOffset", TestType::Single)
                .field("MineFront", class("Vec3"))
                .field("LaserOffset", class("Vec3")),
        )
        .structure(
            trigger_type("SSensorMineProximityTrigger")
                .field("MaxRaysPerMine", TestType::Int32)
                .field("Radius", TestType::Single)
                .field("WarningRadius", TestType::Single)
                .field("Front", class("Vec3")),
        )
        .structure(trigger_type("FixtureFutureSensorTrigger"))
}

fn ordnance_schema(database: DataForgeBuilder, case: Case) -> DataForgeBuilder {
    let emissions = if matches!(case, Case::WrongStorage) {
        pointer("SOrdnanceEmissionsParams")
    } else {
        class("SOrdnanceEmissionsParams")
    };
    database
        .structure(TestStruct::new("ExplosionBaseParams"))
        .structure(
            TestStruct::new("ExplosionParams")
                .parent("ExplosionBaseParams")
                .field("pressure", TestType::Single)
                .field("damage", pointer("DamageBase")),
        )
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single)
                .field("DamageEnergy", TestType::Single),
        )
        .structure(
            TestStruct::new("SOrdnanceEmissionsParams")
                .field("active", TestType::Bool)
                .field("minValue", TestType::Single),
        )
        .structure(
            TestStruct::new("SExplosiveOrdnanceParams")
                .parent("DataForgeComponentParams")
                .field("requiresLauncher", TestType::Bool)
                .field("armTime", TestType::Single)
                .field("maxArmableOverride", TestType::Int32)
                .field("explosionParams", class("ExplosionParams"))
                .array("emissionsParams", emissions),
        )
        .structure(ordnance_type("SCItemBombParams").field("dragAreaRadius", TestType::Single))
        .structure(ordnance_type("SCItemSpaceMineParams"))
        .structure(ordnance_type("SCItemMissileParams"))
}

fn mine(trigger: TestValue) -> TestValue {
    node(
        TestInstance::new("SSensorMineComponentParams")
            .value("FriendlyFire", TestValue::Bool(false))
            .value(
                "LaserVFX",
                node(
                    TestInstance::new("GlobalResourceParticle")
                        .value("path", TestValue::text("fixture_particle")),
                ),
            )
            .value("TriggerType", trigger),
    )
}

fn laser() -> TestInstance {
    TestInstance::new("SSensorMineLaserTrigger")
        .value("LaserLength", TestValue::Single(0.0))
        .value("LaserHitOffset", TestValue::Single(1.5))
        .value("MineFront", vector(-1.0))
}

fn proximity() -> TestInstance {
    TestInstance::new("SSensorMineProximityTrigger")
        .value("MaxRaysPerMine", TestValue::Int32(-1))
        .value("Radius", TestValue::Single(2.0))
        .value("WarningRadius", TestValue::Single(3.0))
        .value("Front", vector(1.0))
}

fn bomb() -> TestValue {
    let blast = TestInstance::new("ExplosionParams")
        .value("pressure", TestValue::Single(f32::INFINITY))
        .value(
            "damage",
            node(
                TestInstance::new("DamageInfo")
                    .value("DamagePhysical", TestValue::Single(1.0))
                    .value("DamageEnergy", TestValue::Single(2.0)),
            ),
        );
    node(
        TestInstance::new("SCItemBombParams")
            .value("requiresLauncher", TestValue::Bool(false))
            .value("armTime", TestValue::Single(0.0))
            .value("maxArmableOverride", TestValue::Int32(-3))
            .value("explosionParams", node(blast))
            .value("dragAreaRadius", TestValue::Single(4.0))
            .value(
                "emissionsParams",
                TestValue::Array(vec![emission(), emission()]),
            ),
    )
}

fn health() -> TestValue {
    node(TestInstance::new("SHealthComponentParams").value(
        "DeathExplosionParams",
        node(TestInstance::new("ExplosionBaseParams")),
    ))
}

fn emission() -> TestValue {
    node(
        TestInstance::new("SOrdnanceEmissionsParams")
            .value("active", TestValue::Bool(false))
            .value("minValue", TestValue::Single(0.0)),
    )
}

fn vector(x: f32) -> TestValue {
    node(TestInstance::new("Vec3").value("x", TestValue::Single(x)))
}

fn trigger_type(name: &str) -> TestStruct {
    TestStruct::new(name).parent("SSensorMineTriggerType")
}

fn ordnance_type(name: &str) -> TestStruct {
    TestStruct::new(name).parent("SExplosiveOrdnanceParams")
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
