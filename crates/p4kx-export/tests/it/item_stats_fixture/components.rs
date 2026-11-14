// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestValue;

use super::values::{damage, instance, single};

fn side(resource: &str, unit: TestValue) -> TestValue {
    instance(
        "ItemResourceGeneration",
        vec![
            ("resource".to_owned(), TestValue::text(resource)),
            ("resourceAmountPerSecond".to_owned(), unit),
        ],
    )
}

fn signature(nominal: f32, decay: f32) -> TestValue {
    instance(
        "Signature",
        vec![
            single("nominalSignature", nominal),
            single("decayRate", decay),
        ],
    )
}

fn resource_state(name: &str, deltas: Vec<TestValue>) -> TestValue {
    instance(
        "ItemResourceComponentParams",
        vec![(
            "states".to_owned(),
            TestValue::Array(vec![instance(
                "ItemResourceState",
                vec![
                    ("name".to_owned(), TestValue::text(name)),
                    (
                        "signatureParams".to_owned(),
                        instance(
                            "SignatureParams",
                            vec![
                                ("EMSignature".to_owned(), signature(500.0, 0.15)),
                                ("IRSignature".to_owned(), signature(80.0, 0.5)),
                            ],
                        ),
                    ),
                    ("deltas".to_owned(), TestValue::Array(deltas)),
                ],
            )]),
        )],
    )
}

pub(super) fn plant_resources(units: f32) -> TestValue {
    resource_state(
        "Online",
        vec![instance(
            "ItemResourceDeltaGeneration",
            vec![(
                "generation".to_owned(),
                side(
                    "Power",
                    instance("SPowerSegmentResourceUnit", vec![single("units", units)]),
                ),
            )],
        )],
    )
}

pub(super) fn cooler_resources(standard: f32) -> TestValue {
    resource_state(
        "Online",
        vec![instance(
            "ItemResourceDeltaConversion",
            vec![
                (
                    "consumption".to_owned(),
                    side(
                        "Power",
                        instance("SPowerSegmentResourceUnit", vec![single("units", 2.0)]),
                    ),
                ),
                (
                    "generation".to_owned(),
                    side(
                        "Coolant",
                        instance(
                            "SStandardResourceUnit",
                            vec![single("standardResourceUnits", standard)],
                        ),
                    ),
                ),
            ],
        )],
    )
}

pub(super) fn shield(max_health: f32) -> TestValue {
    instance(
        "SCItemShieldGeneratorParams",
        vec![
            single("MaxShieldHealth", max_health),
            single("MaxShieldRegen", 160.0),
            single("DecayRatio", 0.25),
            single("DownedRegenDelay", 5.0),
            single("DamagedRegenDelay", 2.5),
        ],
    )
}

pub(super) fn drive(jump_range: f32) -> TestValue {
    instance(
        "SCItemQuantumDriveParams",
        vec![
            single("quantumFuelRequirement", 0.5),
            single("jumpRange", jump_range),
            single("disconnectRange", 20_000.0),
            (
                "params".to_owned(),
                instance(
                    "QuantumDriveParams",
                    vec![
                        single("driveSpeed", 2.0e8),
                        single("cooldownTime", 6.0),
                        single("spoolUpTime", 10.0),
                        single("stageOneAccelRate", 1000.0),
                        single("stageTwoAccelRate", 2000.0),
                        single("engageSpeed", 20.0),
                    ],
                ),
            ),
        ],
    )
}

pub(super) fn missile() -> TestValue {
    instance(
        "SCItemMissileParams",
        vec![
            (
                "explosionParams".to_owned(),
                instance(
                    "ExplosionParams",
                    vec![
                        single("minRadius", 2.0),
                        single("maxRadius", 12.0),
                        ("damage".to_owned(), damage(1500.0, 0.0)),
                    ],
                ),
            ),
            (
                "targetingParams".to_owned(),
                instance(
                    "MissileTargetingParams",
                    vec![
                        ("trackingSignalType".to_owned(), TestValue::text("Infrared")),
                        single("lockTime", 2.0),
                        single("lockingAngle", 45.0),
                        single("lockRangeMin", 100.0),
                        single("lockRangeMax", 5000.0),
                    ],
                ),
            ),
            (
                "GCSParams".to_owned(),
                instance("MissileGcsParams", vec![single("linearSpeed", 300.0)]),
            ),
        ],
    )
}

pub(super) fn health() -> TestValue {
    instance("SHealthComponentParams", vec![single("Health", 450.0)])
}
