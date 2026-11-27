// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const GLOBAL: u128 = 0x1166_0000_0000_4000_8000_0000_0000_d001;
pub(super) const PROCESS: u128 = 0x1166_0000_0000_4000_8000_0000_0000_d002;
pub(super) const RESOURCE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_d003;
pub(super) const MISSING: u128 = 0x1166_0000_0000_4000_8000_0000_0000_d004;
pub(super) const WIDE: i64 = 9_007_199_254_740_993;
pub(super) const NAN_BITS: u32 = 0xffc0_1166;
pub(super) const ROOT_NUMBERS: [&str; 26] = [
    "absorbableVolumeThreshold",
    "cSCUPerVolume",
    "childRockInactivityLifetime",
    "childRockInvulnerabilityTime",
    "childRockLifeTimer",
    "childRockZeroGDamping",
    "compositionMassMultiplier",
    "controlledBreakingDecayRate",
    "controlledBreakingFillRate",
    "controlledBreakingFillRateDanger",
    "dangerBreakingDecayRate",
    "dangerBreakingFillRate",
    "dangerBreakingFillRateExponent",
    "dangerToGadgetDamage",
    "decayPerMass",
    "defaultMass",
    "gadgetDestroyThreshold",
    "gadgetDetachThreshold",
    "modifierPersistenceTime",
    "optimalWindowFactor",
    "optimalWindowMaxSize",
    "optimalWindowSize",
    "optimalWindowThinnessCurveFactor",
    "powerCapacityPerMass",
    "resistanceCurveFactor",
    "terrainFactorStaticThreshold",
];

pub(super) fn database(malformed: bool) -> DataForgeBuilder {
    let builder = schema(malformed);
    builder
        .record(global_record("primary", malformed))
        .record(global_record("alias", malformed))
        .record(process("primary", "Fast", "Careful", "@process_method"))
        .record(process("alias", "", "FutureValue", "@process_missing"))
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "ResourceType.SourceOnly",
            "libs/foundry/records/resourcetypedatabase/source_only.xml",
        ))
        .record(TestRecord::new(
            MISSING + 1,
            "Foreign",
            "Foreign.Unread",
            "libs/foundry/records/unrelated/unread.xml",
        ))
}

pub(super) fn unique_database(malformed: bool) -> DataForgeBuilder {
    schema(malformed)
        .record(global_record("primary", malformed).value("wasteResourceType", TestValue::Null))
}

fn schema(malformed: bool) -> DataForgeBuilder {
    mining_blocks(
        DataForgeBuilder::new()
            .version(8)
            .structure(global_schema(malformed)),
    )
    .structure(
        TestStruct::new("FutureNode")
            .field("value", TestType::UInt64)
            .field("label", TestType::Locale),
    )
    .enumeration("ObservedSpeedLabels", ["Fast", ""])
    .enumeration("ObservedQualityLabels", ["Careful", "FutureValue"])
    .structure(refining_schema())
    .structure(TestStruct::new("ResourceType"))
    .structure(
        TestStruct::new("Foreign")
            .declared_size(1)
            .field("unread", TestType::UInt64),
    )
}

fn global_schema(malformed: bool) -> TestStruct {
    let mut global = TestStruct::new("MiningGlobalParams");
    for (index, field) in ROOT_NUMBERS.into_iter().enumerate() {
        global = global.field(field, numeric_type(index, malformed));
    }
    global
        .field(
            "hitConsistencyParams",
            TestType::Class("ObservedHitData".to_owned()),
        )
        .field(
            "mineableInstabilityParams",
            TestType::StrongPointer("ObservedInstabilityData".to_owned()),
        )
        .field(
            "mineableExplosionParams",
            TestType::Class("ObservedExplosionData".to_owned()),
        )
        .field(
            "wasteResourceType",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
        .array(
            "futureNodes",
            TestType::StrongPointer("FutureNode".to_owned()),
        )
}

fn mining_blocks(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(block(
            "ObservedHitData",
            &[
                "extractionMagnitude",
                "hitHistoryWindow",
                "maxEffectOnInstability",
                "minDeviation",
                "standardDeviationMultiplier",
                "timeExponent",
            ],
        ))
        .structure(block(
            "ObservedInstabilityData",
            &[
                "instabilityCurveFactor",
                "instabilityWavePeriod",
                "instabilityWaveVariance",
            ],
        ))
        .structure(block(
            "ObservedExplosionData",
            &["dangerPoolFactor", "defaultVolume"],
        ))
}

fn refining_schema() -> TestStruct {
    TestStruct::new("RefiningProcess")
        .field("processName", TestType::Locale)
        .field(
            "refiningSpeed",
            TestType::Enum("ObservedSpeedLabels".to_owned()),
        )
        .field(
            "refiningQuality",
            TestType::Enum("ObservedQualityLabels".to_owned()),
        )
        .field("unprojectedFlag", TestType::Bool)
}

const fn numeric_type(index: usize, malformed: bool) -> TestType {
    match index {
        0 if malformed => TestType::Bool,
        0 => TestType::UInt64,
        2 => TestType::Int64,
        4 => TestType::Double,
        _ => TestType::Single,
    }
}

const fn numeric_value(index: usize, malformed: bool) -> TestValue {
    match index {
        0 if malformed => TestValue::Bool(false),
        0 => TestValue::UInt64(u64::MAX),
        1 => TestValue::Single(0.85),
        2 => TestValue::Int64(WIDE),
        3 => TestValue::Single(f32::from_bits(NAN_BITS)),
        4 => TestValue::Double(1.0e300),
        5 => TestValue::Single(-0.0),
        _ => TestValue::Single(0.5),
    }
}

fn block(name: &str, fields: &[&str]) -> TestStruct {
    fields.iter().fold(TestStruct::new(name), |value, &field| {
        value.field(field, TestType::Single)
    })
}

fn global_record(name: &str, malformed: bool) -> TestRecord {
    let mut record = TestRecord::new(
        GLOBAL,
        "MiningGlobalParams",
        format!("MiningGlobalParams.{name}"),
        format!("libs/foundry/records/mining/miningglobalparams_{name}.xml"),
    );
    for (index, field) in ROOT_NUMBERS.into_iter().enumerate() {
        record = record.value(field, numeric_value(index, malformed));
    }
    record
        .value(
            "hitConsistencyParams",
            TestValue::Struct(
                TestInstance::new("ObservedHitData").value("timeExponent", TestValue::Single(2.0)),
            ),
        )
        .value(
            "mineableInstabilityParams",
            TestValue::Struct(
                TestInstance::new("ObservedInstabilityData")
                    .value("instabilityWavePeriod", TestValue::Single(3.0)),
            ),
        )
        .value(
            "mineableExplosionParams",
            TestValue::Struct(
                TestInstance::new("ObservedExplosionData")
                    .value("defaultVolume", TestValue::Single(5.0)),
            ),
        )
        .value("wasteResourceType", TestValue::Reference(MISSING))
        .value(
            "futureNodes",
            TestValue::Array(vec![future_node(), TestValue::Null, future_node()]),
        )
}

fn future_node() -> TestValue {
    TestValue::Struct(
        TestInstance::new("FutureNode")
            .value("value", TestValue::UInt64(u64::MAX))
            .value("label", TestValue::text("@retained_future_label")),
    )
}

fn process(name: &str, speed: &str, quality: &str, key: &str) -> TestRecord {
    TestRecord::new(
        PROCESS,
        "RefiningProcess",
        format!("RefiningProcess.{name}"),
        format!("libs/foundry/records/refiningprocess/{name}.xml"),
    )
    .value("processName", TestValue::text(key))
    .value("refiningSpeed", TestValue::text(speed))
    .value("refiningQuality", TestValue::text(quality))
    .value("unprojectedFlag", TestValue::Bool(false))
}
