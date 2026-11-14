// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const BULLET_AMMO: u128 = 0x8101;
const TACHYON_AMMO: u128 = 0x8102;
const UNSUPPORTED_AMMO: u128 = 0x8103;
const EMPTY_AMMO: u128 = 0x8104;
const DAMAGE_MACRO: u128 = 0x8105;
const CHAFF_AMMO: u128 = 0x8106;
const FLARE_AMMO: u128 = 0x8107;

pub(super) fn bytes() -> Vec<u8> {
    let builder = projectile_types(DataForgeBuilder::new().version(8));
    let builder = bullet_types(builder);
    let builder = behavior_types(builder);
    countermeasure_types(presentation_types(builder))
        .record(ammo(BULLET_AMMO, "bullet", node(bullet())))
        .record(ammo(TACHYON_AMMO, "tachyon", node(tachyon())))
        .record(ammo(
            UNSUPPORTED_AMMO,
            "unsupported",
            node(TestInstance::new("FixtureOtherProjectile")),
        ))
        .record(ammo(EMPTY_AMMO, "empty", TestValue::Null))
        .record(TestRecord::new(
            DAMAGE_MACRO,
            "DamageMacro",
            "DamageMacro.Fixture",
            "fixture/damage_macro.xml",
        ))
        .record(ammo(
            CHAFF_AMMO,
            "chaff",
            node(countermeasure("CounterMeasureChaffParams")),
        ))
        .record(ammo(
            FLARE_AMMO,
            "flare",
            node(countermeasure("CounterMeasureFlareParams")),
        ))
        .build()
}

fn ammo(id: u128, name: &str, projectile: TestValue) -> TestRecord {
    TestRecord::new(
        id,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("fixture/{name}.xml"),
    )
    .value("projectileParams", projectile)
}

fn projectile_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("AmmoParams").field("projectileParams", pointer("ProjectileParams")),
        )
        .structure(
            TestStruct::new("ProjectileParams")
                .field("detonationParams", pointer("ProjectileDetonationParams"))
                .field(
                    "proximityTriggerParams",
                    pointer("ProjectileProximityTriggerParams"),
                ),
        )
        .structure(
            TestStruct::new("ProjectileDetonationParams")
                .field("explodeOnFinalImpact", TestType::Bool)
                .field("explosionParams", class("ExplosionParams")),
        )
        .structure(singles("ProjectileProximityTriggerParams", &["radius"]))
        .structure(
            singles("ExplosionParams", &["minRadius"]).field("damage", pointer("DamageBase")),
        )
        .structure(
            singles(
                "TachyonProjectileParams",
                &["fullDamageRange", "zeroDamageRange"],
            )
            .parent("ProjectileParams")
            .field("damage", pointer("DamageBase"))
            .field("penetrationParams", class("AmmoPenetrationParams"))
            .array("beamEffects", class("SBeamEffectParams")),
        )
        .structure(TestStruct::new("FixtureOtherProjectile").parent("ProjectileParams"))
        .structure(TestStruct::new("DamageBase"))
        .structure(singles("DamageInfo", &["DamagePhysical"]).parent("DamageBase"))
        .structure(
            singles("DamageParams", &["damageTotal"])
                .parent("DamageBase")
                .field("damageMacro", reference("DamageMacro")),
        )
        .structure(TestStruct::new("DamageMacro"))
}

fn bullet_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            singles(
                "BulletProjectileParams",
                &["impactRadius", "minImpactRadius"],
            )
            .parent("ProjectileParams")
            .field("damage", pointer("DamageBase"))
            .field("keepAliveOnZeroDamage", TestType::Bool)
            .field("damageDropParams", pointer("BulletDamageDropParams"))
            .field(
                "impulseFalloffParams",
                pointer("BulletImpulseFalloffParams"),
            )
            .field("pierceabilityParams", class("BulletPierceabilityParams"))
            .field("penetrationParams", class("AmmoPenetrationParams"))
            .field("electronParams", pointer("BulletElectronParams"))
            .field("visualParams", class("BulletVisualParams"))
            .field("alternateVisualParams", pointer("BulletVisualParams"))
            .field(
                "additionalProjectilesParams",
                pointer("AdditionalProjectilesParams"),
            )
            .array("hitBehaviors", pointer("HitBehaviorBase")),
        )
        .structure(
            TestStruct::new("BulletDamageDropParams")
                .field("damageDropMinDistance", pointer("DamageBase"))
                .field("damageDropPerMeter", pointer("DamageBase"))
                .field("damageDropMinDamage", pointer("DamageBase")),
        )
        .structure(singles("BulletImpulseFalloffParams", &["maxFalloff"]))
        .structure(singles(
            "BulletPierceabilityParams",
            &["damageFalloffLevel2"],
        ))
        .structure(singles("AmmoPenetrationParams", &["nearRadius"]))
        .structure(TestStruct::new("BulletElectronParams").field("maximumJumps", TestType::Int32))
}

fn behavior_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("AdditionalProjectilesParams")
                .field("ammoRecord", reference("AmmoParams"))
                .field("amount", TestType::Int32)
                .field("lifetimeRange", pointer("Range")),
        )
        .structure(singles("Range", &["minimum", "maximum"]))
        .structure(TestStruct::new("HitBehaviorBase"))
        .structure(
            singles("HitBehaviorDamageOverTime", &["duration"])
                .parent("HitBehaviorBase")
                .field("damagePerSecond", pointer("DamageBase"))
                .field("ignoreShields", TestType::Bool),
        )
}

fn presentation_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("GlobalResourceParticle").field("path", TestType::String))
        .structure(TestStruct::new("GlobalResourceAudio").field("audioTrigger", TestType::String))
        .structure(TestStruct::new("GlobalResourceMaterial").field("path", TestType::String))
        .structure(
            TestStruct::new("BulletVisualParams")
                .field("Material", class("GlobalResourceMaterial")),
        )
        .structure(TestStruct::new("SBeamEffectParams").field("helper", TestType::String))
}

fn countermeasure_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("CounterMeasureProjectileParams")
                .parent("ProjectileParams")
                .field("typeParams", pointer("CounterMeasureBaseParams"))
                .field("trailStartAudioTrigger", class("GlobalResourceAudio")),
        )
        .structure(TestStruct::new("CounterMeasureBaseParams"))
        .structure(
            singles("CounterMeasureChaffParams", &["StartInfrared"])
                .parent("CounterMeasureBaseParams")
                .field("radiusRange", class("Range")),
        )
        .structure(
            singles("CounterMeasureFlareParams", &["StartInfrared"])
                .parent("CounterMeasureBaseParams"),
        )
}

fn visual(path: &str) -> TestValue {
    node(TestInstance::new("BulletVisualParams").value(
        "Material",
        node(TestInstance::new("GlobalResourceMaterial").value("path", TestValue::text(path))),
    ))
}

fn countermeasure(kind: &str) -> TestInstance {
    TestInstance::new("CounterMeasureProjectileParams")
        .value(
            "typeParams",
            node(TestInstance::new(kind).value("StartInfrared", TestValue::Single(1.0))),
        )
        .value(
            "trailStartAudioTrigger",
            node(
                TestInstance::new("GlobalResourceAudio")
                    .value("audioTrigger", TestValue::text("fixture-trigger")),
            ),
        )
}

fn bullet() -> TestInstance {
    TestInstance::new("BulletProjectileParams")
        .value("damage", node(info(1.0)))
        .value("impactRadius", TestValue::Single(-0.0))
        .value("keepAliveOnZeroDamage", TestValue::Bool(false))
        .value("detonationParams", node(detonation()))
        .value(
            "proximityTriggerParams",
            node(
                TestInstance::new("ProjectileProximityTriggerParams")
                    .value("radius", TestValue::Single(2.0)),
            ),
        )
        .value("damageDropParams", node(damage_drop()))
        .value(
            "impulseFalloffParams",
            node(
                TestInstance::new("BulletImpulseFalloffParams")
                    .value("maxFalloff", TestValue::Single(0.0)),
            ),
        )
        .value(
            "pierceabilityParams",
            node(
                TestInstance::new("BulletPierceabilityParams")
                    .value("damageFalloffLevel2", TestValue::Single(1.0)),
            ),
        )
        .value("penetrationParams", node(penetration()))
        .value(
            "electronParams",
            node(
                TestInstance::new("BulletElectronParams")
                    .value("maximumJumps", TestValue::Int32(-2)),
            ),
        )
        .value("visualParams", visual("fixture/normal.mtl"))
        .value("alternateVisualParams", visual("fixture/alternate.mtl"))
        .value("additionalProjectilesParams", node(additional()))
        .value("hitBehaviors", hits())
}

fn tachyon() -> TestInstance {
    let beam = || {
        node(
            TestInstance::new("SBeamEffectParams").value("helper", TestValue::text("fixture-beam")),
        )
    };
    TestInstance::new("TachyonProjectileParams")
        .value("damage", node(info(2.0)))
        .value("fullDamageRange", TestValue::Single(0.0))
        .value("zeroDamageRange", TestValue::Single(2.0))
        .value("penetrationParams", node(penetration()))
        .value("beamEffects", TestValue::Array(vec![beam(), beam()]))
}

fn detonation() -> TestInstance {
    TestInstance::new("ProjectileDetonationParams")
        .value("explodeOnFinalImpact", TestValue::Bool(true))
        .value(
            "explosionParams",
            node(
                TestInstance::new("ExplosionParams")
                    .value("minRadius", TestValue::Single(1.0))
                    .value("damage", node(info(2.0))),
            ),
        )
}

fn damage_drop() -> TestInstance {
    let per_meter = TestInstance::new("DamageParams")
        .value("damageMacro", TestValue::Reference(DAMAGE_MACRO))
        .value(
            "damageTotal",
            TestValue::Single(f32::from_bits(0x7fc0_1234)),
        );
    TestInstance::new("BulletDamageDropParams")
        .value(
            "damageDropMinDistance",
            node(TestInstance::new("DamageBase")),
        )
        .value("damageDropPerMeter", node(per_meter))
        .value("damageDropMinDamage", node(info(3.0)))
}

fn info(physical: f32) -> TestInstance {
    TestInstance::new("DamageInfo").value("DamagePhysical", TestValue::Single(physical))
}

fn penetration() -> TestInstance {
    TestInstance::new("AmmoPenetrationParams").value("nearRadius", TestValue::Single(0.5))
}

fn additional() -> TestInstance {
    TestInstance::new("AdditionalProjectilesParams")
        .value("ammoRecord", TestValue::Reference(BULLET_AMMO))
        .value("amount", TestValue::Int32(0))
}

fn hits() -> TestValue {
    TestValue::Array(vec![
        node(
            TestInstance::new("HitBehaviorDamageOverTime")
                .value("damagePerSecond", node(info(4.0)))
                .value("duration", TestValue::Single(1.0))
                .value("ignoreShields", TestValue::Bool(false)),
        ),
        TestValue::Null,
        node(TestInstance::new("HitBehaviorBase")),
    ])
}

fn node(source: TestInstance) -> TestValue {
    TestValue::Struct(source)
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn singles(name: &str, fields: &[&str]) -> TestStruct {
    fields.iter().fold(TestStruct::new(name), |shape, field| {
        shape.field(*field, TestType::Single)
    })
}
