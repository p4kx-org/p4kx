// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::super::{HEALTH_ONLY, item};

pub(super) const TARGET: u128 = 0x7511;
pub(super) const WRONG: u128 = 0x7512;
pub(super) const MISSING: u128 = 0x75ff;

pub(super) fn database(unknown: bool) -> DataForgeBuilder {
    let mut slots = vec![
        health(node(inline())),
        health(node(TestInstance::new("ExplosionBaseParams"))),
        health(link(TARGET)),
        health(link(MISSING)),
        health(link(WRONG)),
        health(TestValue::Null),
    ];
    if unknown {
        slots.push(health(node(TestInstance::new("FixtureExplosionUnknown"))));
    }
    schema()
        .record(item(HEALTH_ONLY, "fixture_death_explosion", "Misc", slots))
        .record(
            TestRecord::new(
                TARGET,
                "ExplosionRecord",
                "ExplosionRecord.Fixture",
                "libs/foundry/records/explosions/fixture.xml",
            )
            .value(
                "explosionParams",
                node(TestInstance::new("ExplosionParams")),
            ),
        )
        .record(TestRecord::new(
            WRONG,
            "FixtureWrongTarget",
            "FixtureWrongTarget.Explosion",
            "libs/foundry/records/explosions/wrong.xml",
        ))
}

fn schema() -> DataForgeBuilder {
    let health = TestStruct::new("SHealthComponentParams")
        .parent("DataForgeComponentParams")
        .field("DeathExplosionParams", pointer("ExplosionBaseParams"));
    let target =
        TestStruct::new("ExplosionRecord").field("explosionParams", class("ExplosionParams"));
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Misc"])
        .enumeration("FixtureFriendlyFire", ["FixtureRule"])
        .enumeration("FixtureShockType", ["FixtureShock"])
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
        .structure(TestStruct::new("SItemDefinition").field("Type", enumeration("FixtureItemType")))
        .structure(health)
        .structure(TestStruct::new("ExplosionBaseParams"))
        .structure(inline_schema())
        .structure(explosion_kind("ExplosionByRecordParams").field(
            "record",
            TestType::ReferenceTo("ExplosionRecord".to_owned()),
        ))
        .structure(explosion_kind("FixtureExplosionUnknown"))
        .structure(target)
        .structure(TestStruct::new("FixtureWrongTarget"))
        .structure(
            TestStruct::new("ExplosionFlashbangParams").field("blindAmount", TestType::Single),
        )
        .structure(TestStruct::new("Vec3").field("x", TestType::Single))
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single)
                .field("DamageEnergy", TestType::Single),
        )
        .structure(
            TestStruct::new("ShockwaveParams")
                .field("primitiveType", enumeration("FixtureShockType")),
        )
        .structure(TestStruct::new("ExplosiveFragmentParams").field("amount", TestType::Int32))
        .structure(TestStruct::new("GlobalResourceParticle").field("path", TestType::String))
}

fn inline_schema() -> TestStruct {
    explosion_kind("ExplosionParams")
        .field("friendlyFire", enumeration("FixtureFriendlyFire"))
        .field("minRadius", TestType::Single)
        .field("pressure", TestType::Single)
        .field("explosionFront", class("Vec3"))
        .field("useRandomScale", TestType::Bool)
        .field("damage", pointer("DamageBase"))
        .field("flashbangParams", pointer("ExplosionFlashbangParams"))
        .field("shockwaveParams", pointer("ShockwaveParams"))
        .field("fragmentParams", pointer("ExplosiveFragmentParams"))
        .field("effect", class("GlobalResourceParticle"))
}

fn inline() -> TestInstance {
    TestInstance::new("ExplosionParams")
        .value("friendlyFire", TestValue::text("FixtureRule"))
        .value("minRadius", TestValue::Single(0.0))
        .value("pressure", TestValue::Single(f32::INFINITY))
        .value(
            "explosionFront",
            node(TestInstance::new("Vec3").value("x", TestValue::Single(-1.0))),
        )
        .value("useRandomScale", TestValue::Bool(false))
        .value(
            "damage",
            node(
                TestInstance::new("DamageInfo")
                    .value("DamagePhysical", TestValue::Single(1.0))
                    .value("DamageEnergy", TestValue::Single(2.0)),
            ),
        )
        .value("flashbangParams", TestValue::Null)
        .value(
            "shockwaveParams",
            node(
                TestInstance::new("ShockwaveParams")
                    .value("primitiveType", TestValue::text("FixtureShock")),
            ),
        )
        .value(
            "fragmentParams",
            node(
                TestInstance::new("ExplosiveFragmentParams").value("amount", TestValue::Int32(-2)),
            ),
        )
        .value(
            "effect",
            node(
                TestInstance::new("GlobalResourceParticle")
                    .value("path", TestValue::text("fixture_particle")),
            ),
        )
}

fn explosion_kind(name: &str) -> TestStruct {
    TestStruct::new(name).parent("ExplosionBaseParams")
}

fn health(value: TestValue) -> TestValue {
    node(TestInstance::new("SHealthComponentParams").value("DeathExplosionParams", value))
}

fn link(guid: u128) -> TestValue {
    node(TestInstance::new("ExplosionByRecordParams").value("record", TestValue::Reference(guid)))
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

fn enumeration(name: &str) -> TestType {
    TestType::Enum(name.to_owned())
}
