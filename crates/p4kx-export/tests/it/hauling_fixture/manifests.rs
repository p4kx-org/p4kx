// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The cargo manifest Records of the hauling fixture.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::{
    CRATE, CRATES, GOLD, MANIFEST_CUSTOM, MANIFEST_EVEN, MANIFEST_RANDOM, SCRAP, array, int,
    record, reference, single, structure,
};

pub(super) fn manifest_even() -> TestRecord {
    let resource = |guid: u128, probability: f32| {
        structure(
            TestInstance::new("CargoResource")
                .value("probability", single(probability))
                .value("resource", reference(guid)),
        )
    };
    let rule = |kind: &str, field: &str, algorithm: TestInstance| {
        structure(
            TestInstance::new(kind)
                .value("probability", single(1.0))
                .value(field, array(vec![reference(CRATE), reference(CRATES)]))
                .value("quantityAlgorithmType", structure(algorithm)),
        )
    };
    record(
        MANIFEST_EVEN,
        "CargoManifest",
        "LegalCargo_Even",
        "cargomanifest/civilian/legalcargo_even",
    )
    .value("lockCargoGridsOnSpawn", TestValue::Bool(false))
    .value(
        "cargoFillCapacity",
        structure(
            TestInstance::new("CargoFillCapacityValue_Even")
                .value("minCapacityRange", single(0.45))
                .value("maxCapacityRange", single(0.5))
                .value(
                    "resources",
                    array(vec![resource(SCRAP, 1.0), resource(GOLD, 0.25)]),
                ),
        ),
    )
    .value(
        "cargoSpawnRules",
        array(vec![
            rule(
                "CargoSpawnEntityDef",
                "entities",
                TestInstance::new("CargoManifestFixedQuantityDef").value("quantity", int(30)),
            ),
            rule(
                "CargoSpawnResourceDef",
                "resources",
                TestInstance::new("CargoManifestProportionQuantityDef")
                    .value("proportion", single(0.5)),
            ),
        ]),
    )
}

pub(super) fn manifest_custom() -> TestRecord {
    let allocation = structure(
        TestInstance::new("CargoResourceAllocation")
            .value("minResourceAllocationPercentage", single(0.03))
            .value("maxResourceAllocationPercentage", single(0.06))
            .value(
                "resources",
                structure(
                    TestInstance::new("CargoResource")
                        .value("probability", single(1.0))
                        .value("resource", reference(GOLD)),
                ),
            ),
    );
    record(
        MANIFEST_CUSTOM,
        "CargoManifest",
        "Pyro_Custom",
        "cargomanifest/pyro/pyro_custom",
    )
    .value(
        "cargoFillCapacity",
        structure(
            TestInstance::new("CargoFillCapacityValue_Custom")
                .value("resources", array(vec![allocation])),
        ),
    )
}

pub(super) fn manifest_random() -> TestRecord {
    record(
        MANIFEST_RANDOM,
        "CargoManifest",
        "RandomFill",
        "cargomanifest/civilian/random_fill",
    )
    .value(
        "tags",
        array(vec![TestValue::text("synthetic-authored-tag")]),
    )
    .value(
        "descriptionTags",
        array(vec![TestValue::text("@synthetic_manifest_description")]),
    )
    .value(
        "cargoFillCapacity",
        structure(
            TestInstance::new("CargoFillCapacityValue_Random")
                .value("minCapacityRange", single(-0.0))
                .value("maxCapacityRange", single(f32::from_bits(0x7fc0_002a))),
        ),
    )
    .value(
        "cargoSpawnRules",
        array(vec![structure(
            TestInstance::new("CargoSpawnFutureDef")
                .value("probability", single(0.75))
                .value("futureQuota", TestValue::UInt64(u64::MAX)),
        )]),
    )
}
