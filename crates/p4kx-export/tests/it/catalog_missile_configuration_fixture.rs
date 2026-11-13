// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ITEM: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ORDNANCE: &str = "SExplosiveOrdnanceParams";
const EMISSION: &str = "SOrdnanceEmissionsParams";
const MISSILE: &str = "SCItemMissileParams";
const GUIDANCE: &str = "SGuidanceAndControlParams";
const PHASE: &str = "SGuidanceAndControlPhaseParams";
const TARGETING: &str = "STargetingParams";
const SIGNAL: &str = "ESignatureType";
const SHAKE: &str = "SShakeParams";
const VECTOR: &str = "Vec3";
const ANGLES: &str = "Ang3";

pub(super) const OWNER: u128 = 0x7101;
pub(super) const LINK: u128 = 0x7102;
pub(super) const MISSING: u128 = 0x7103;

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Exact,
    WrongTargetingClass,
    WrongSignalEnum,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SIGNAL, ["fixtureFutureSignal"])
        .enumeration("FixtureWrongSignal", ["fixtureFutureSignal"])
        .structure(
            TestStruct::new(ITEM)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(ordnance())
        .structure(
            TestStruct::new(EMISSION)
                .field("active", TestType::Bool)
                .field("minValue", TestType::Single),
        )
        .structure(missile(layout))
        .structure(guidance())
        .structure(phase())
        .structure(targeting(layout))
        .structure(
            TestStruct::new("FixtureAlternativeTargeting")
                .field("fixtureAlternate", TestType::Bool),
        )
        .structure(shake())
        .structure(axes(VECTOR))
        .structure(axes(ANGLES))
        .structure(TestStruct::new("FixtureLaunchHint"))
        .record(TestRecord::new(
            LINK,
            "FixtureLaunchHint",
            "FixtureLaunchHint.Present",
            "fixture/missile/hint.xml",
        ))
        .record(
            TestRecord::new(
                OWNER,
                ITEM,
                "EntityClassDefinition.FixtureMissile",
                "libs/foundry/records/entities/scitem/fixture_missile.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Null,
                    component(layout, LINK, true),
                    component(layout, MISSING, false),
                ]),
            ),
        )
}

fn missile(layout: Layout) -> TestStruct {
    TestStruct::new(MISSILE)
        .parent(ORDNANCE)
        .field("GCSParams", class(GUIDANCE))
        .field(
            "targetingParams",
            class(if matches!(layout, Layout::WrongTargetingClass) {
                "FixtureAlternativeTargeting"
            } else {
                TARGETING
            }),
        )
        .field("shakeParams", TestType::StrongPointer(SHAKE.to_owned()))
        .field("fixtureUnknownRoot", TestType::Bool)
}

fn ordnance() -> TestStruct {
    TestStruct::new(ORDNANCE)
        .parent(COMPONENT)
        .field("requiresLauncher", TestType::Bool)
        .field("enableLifetime", TestType::Bool)
        .field("projectileProximity", TestType::Single)
        .array("emissionsParams", class(EMISSION))
}

fn shake() -> TestStruct {
    TestStruct::new(SHAKE)
        .field("offsetAngle", class(ANGLES))
        .field("offsetPosition", class(VECTOR))
        .field("randomness", TestType::Single)
        .field("range", TestType::Single)
        .field("duration", TestType::Single)
        .field("frequency", TestType::Single)
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn axes(name: &str) -> TestStruct {
    TestStruct::new(name)
        .field("x", TestType::Single)
        .field("y", TestType::Single)
        .field("z", TestType::Single)
}

fn guidance() -> TestStruct {
    TestStruct::new(GUIDANCE)
        .field("isDumbMissile", TestType::Bool)
        .field("linearSpeed", TestType::Single)
        .field("pidIntegralTerm", TestType::Single)
        .field("pidDerivativeTerm", TestType::Single)
        .field("pidProportionalTerm", TestType::Single)
        .field("boostPhase", class(PHASE))
        .field("interceptPhase", class(PHASE))
        .field("terminalPhase", class(PHASE))
}

fn phase() -> TestStruct {
    TestStruct::new(PHASE)
        .field("angularSpeed", TestType::Single)
        .field("maxRotationAccel", TestType::Single)
        .field("pidAggression", TestType::Single)
        .field("maxLinearAccelerationPositive", class(VECTOR))
        .field("maxLinearAccelerationNegative", class(VECTOR))
}

fn targeting(layout: Layout) -> TestStruct {
    let signal = if matches!(layout, Layout::WrongSignalEnum) {
        "FixtureWrongSignal"
    } else {
        SIGNAL
    };
    TestStruct::new(TARGETING)
        .field("trackingSignalType", TestType::Enum(signal.to_owned()))
        .field("allowDumbFiring", TestType::Bool)
        .field("lockRangeMin", TestType::Single)
        .field("maxTimesCanMiss", TestType::Int32)
        .field(
            "fixtureLaunchHint",
            TestType::ReferenceTo("FixtureLaunchHint".to_owned()),
        )
}

fn component(layout: Layout, link: u128, has_shake: bool) -> TestValue {
    let targeting = if matches!(layout, Layout::WrongTargetingClass) {
        TestInstance::new("FixtureAlternativeTargeting")
            .value("fixtureAlternate", TestValue::Bool(false))
    } else {
        TestInstance::new(TARGETING)
            .value("trackingSignalType", TestValue::text("fixtureFutureSignal"))
            .value("allowDumbFiring", TestValue::Bool(false))
            .value("lockRangeMin", TestValue::Single(-0.0))
            .value("maxTimesCanMiss", TestValue::Int32(0))
            .value("fixtureLaunchHint", TestValue::Reference(link))
    };
    let shake = if has_shake {
        TestValue::Struct(
            TestInstance::new(SHAKE)
                .value("offsetAngle", vector(ANGLES, 60.0))
                .value("offsetPosition", vector(VECTOR, 70.0))
                .value("frequency", TestValue::Single(3.0)),
        )
    } else {
        TestValue::Null
    };
    TestValue::Struct(
        TestInstance::new(MISSILE)
            .value("GCSParams", guidance_value())
            .value("targetingParams", TestValue::Struct(targeting))
            .value("shakeParams", shake)
            .value("requiresLauncher", TestValue::Bool(false))
            .value("enableLifetime", TestValue::Bool(true))
            .value("projectileProximity", TestValue::Single(0.0))
            .value(
                "emissionsParams",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(EMISSION)
                        .value("active", TestValue::Bool(false))
                        .value("minValue", TestValue::Single(7.0)),
                )]),
            )
            .value("fixtureUnknownRoot", TestValue::Bool(false)),
    )
}

fn guidance_value() -> TestValue {
    TestValue::Struct(
        TestInstance::new(GUIDANCE)
            .value("isDumbMissile", TestValue::Bool(false))
            .value("linearSpeed", TestValue::Single(512.0))
            .value("pidIntegralTerm", TestValue::Single(11.0))
            .value("pidDerivativeTerm", TestValue::Single(12.0))
            .value("pidProportionalTerm", TestValue::Single(13.0))
            .value("boostPhase", phase_value(10.0))
            .value("interceptPhase", phase_value(20.0))
            .value("terminalPhase", phase_value(30.0)),
    )
}

fn phase_value(seed: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PHASE)
            .value("angularSpeed", TestValue::Single(seed))
            .value("maxRotationAccel", TestValue::Single(seed + 4.0))
            .value("pidAggression", TestValue::Single(seed + 5.0))
            .value("maxLinearAccelerationPositive", vector(VECTOR, seed + 1.0))
            .value("maxLinearAccelerationNegative", vector(VECTOR, -seed - 1.0)),
    )
}

fn vector(name: &str, seed: f32) -> TestValue {
    let step = seed.signum();
    TestValue::Struct(
        TestInstance::new(name)
            .value("x", TestValue::Single(seed))
            .value("y", TestValue::Single(seed + step))
            .value("z", TestValue::Single(seed + step + step)),
    )
}
