// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

const RESISTANCE_NAMES: &[&str] = &[
    "PhysicalResistance",
    "EnergyResistance",
    "DistortionResistance",
    "ThermalResistance",
    "BiochemicalResistance",
    "StunResistance",
];

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Misc"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(health_type())
        .structure(TestStruct::new("DamageResistanceBase"))
        .structure(resistance_type())
        .structure(singles(
            TestStruct::new("DamageResistanceEntry"),
            &["Multiplier", "Threshold", "DamageCap", "DamageCapLimit"],
        ))
        .structure(distortion_type())
        .structure(
            component("SDegradationParams")
                .field("StopDegradingIfDestroyed", TestType::Bool)
                .array("accumulators", pointer("SAccumulatorParams")),
        )
        .structure(
            TestStruct::new("SAccumulatorParams")
                .field("InitialAccumulationRatio", TestType::Single),
        )
        .structure(
            accumulator("SWearAccumulatorParams")
                .field("MaxLifetimeHours", TestType::Single)
                .field("UseAsTimer", TestType::Bool)
                .field("EffectCurve", class("BezierCurve")),
        )
        .structure(
            accumulator("SDirtAccumulatorParams").field("InitialDirtRatio", TestType::Single),
        )
        .structure(accumulator("SWetnessAccumulatorParams"))
        .structure(accumulator("FixtureUnknownAccumulator").field("uninterpreted", TestType::Bool))
        .structure(
            TestStruct::new("BezierCurve")
                .field("useLUT", TestType::Bool)
                .array("points", class("Vec2")),
        )
        .structure(singles(TestStruct::new("Vec2"), &["x", "y"]))
}

pub(super) fn health() -> TestValue {
    TestValue::Struct(
        TestInstance::new("SHealthComponentParams")
            .value("Health", TestValue::Single(1.0))
            .value("DamageCap", TestValue::Single(0.0))
            .value("ExplosionDelayTime", TestValue::Single(f32::NEG_INFINITY))
            .value("IsSalvagable", TestValue::Bool(true))
            .value("IsRepairable", TestValue::Bool(false))
            .value("MirrorSalvageHit", TestValue::Bool(false))
            .value("DamageResistances", resistance()),
    )
}

pub(super) fn distortion() -> TestValue {
    TestValue::Struct(
        TestInstance::new("SDistortionParams")
            .value("Maximum", TestValue::Single(10.0))
            .value("PowerChangeOnlyAtMaxDistortion", TestValue::Bool(false)),
    )
}

pub(super) fn degradation(unsupported: bool) -> TestValue {
    let second = if unsupported {
        TestInstance::new("FixtureUnknownAccumulator").value("uninterpreted", TestValue::Bool(true))
    } else {
        TestInstance::new("SDirtAccumulatorParams")
            .value("InitialDirtRatio", TestValue::Single(0.0))
    };
    TestValue::Struct(
        TestInstance::new("SDegradationParams")
            .value("StopDegradingIfDestroyed", TestValue::Bool(false))
            .value(
                "accumulators",
                TestValue::Array(vec![
                    wear(),
                    TestValue::Struct(second),
                    TestValue::Struct(TestInstance::new("SWetnessAccumulatorParams")),
                    wear(),
                ]),
            ),
    )
}

fn health_type() -> TestStruct {
    singles(
        component("SHealthComponentParams"),
        &["Health", "DamageCap", "ExplosionDelayTime"],
    )
    .field("IsSalvagable", TestType::Bool)
    .field("IsRepairable", TestType::Bool)
    .field("MirrorSalvageHit", TestType::Bool)
    .field("DamageResistances", pointer("DamageResistanceBase"))
}

fn distortion_type() -> TestStruct {
    singles(
        component("SDistortionParams"),
        &[
            "DecayDelay",
            "DecayRate",
            "Maximum",
            "WarningRatio",
            "RecoveryRatio",
            "PowerRatioAtMaxDistortion",
        ],
    )
    .field("PowerChangeOnlyAtMaxDistortion", TestType::Bool)
}

fn resistance_type() -> TestStruct {
    RESISTANCE_NAMES.iter().fold(
        TestStruct::new("DamageResistance")
            .parent("DamageResistanceBase")
            .field("IgnoreMeleeDamage", TestType::Bool),
        |source, &name| source.field(name, class("DamageResistanceEntry")),
    )
}

fn resistance() -> TestValue {
    TestValue::Struct(RESISTANCE_NAMES.iter().fold(
        TestInstance::new("DamageResistance").value("IgnoreMeleeDamage", TestValue::Bool(false)),
        |source, &name| {
            source.value(
                name,
                TestValue::Struct(
                    TestInstance::new("DamageResistanceEntry")
                        .value("Multiplier", TestValue::Single(1.0))
                        .value("Threshold", TestValue::Single(0.0))
                        .value("DamageCap", TestValue::Single(2.0))
                        .value("DamageCapLimit", TestValue::Single(3.0)),
                ),
            )
        },
    ))
}

fn wear() -> TestValue {
    let curve = TestInstance::new("BezierCurve")
        .value("useLUT", TestValue::Bool(false))
        .value(
            "points",
            TestValue::Array(vec![point(1.0, 2.0), point(0.0, 3.0)]),
        );
    TestValue::Struct(
        TestInstance::new("SWearAccumulatorParams")
            .value("MaxLifetimeHours", TestValue::Single(10.0))
            .value("UseAsTimer", TestValue::Bool(false))
            .value("EffectCurve", TestValue::Struct(curve)),
    )
}

fn point(x: f32, y: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Vec2")
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y)),
    )
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn accumulator(name: &str) -> TestStruct {
    TestStruct::new(name).parent("SAccumulatorParams")
}

fn singles(source: TestStruct, fields: &[&str]) -> TestStruct {
    fields.iter().fold(source, |source, &field| {
        source.field(field, TestType::Single)
    })
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
