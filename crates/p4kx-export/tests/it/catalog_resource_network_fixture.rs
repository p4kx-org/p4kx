// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const FIRST: u128 = 0x1059_0100;
pub(super) const SECOND: u128 = 0x1059_0101;
pub(super) const OTHER: u128 = 0x1059_0102;
pub(super) const NULL: u128 = 0x1059_0103;
pub(super) const THRESHOLD_BITS: u32 = 0x7fc0_1059;
pub(super) const RAW_BITS: u64 = 0x7ff8_0000_0000_1059;

const MODE: &str = "GameMode";
const MODE_ID: &str = "EGameModeId";
const PARAMS: &str = "SIParamsModule";
const STANDARD: &str = "SStandardParams";
const OTHER_PARAMS: &str = "FixtureAlternativeMode";
const RULES: &str = "ResourceNetworkParams";
const RADAR: &str = "RadarOptionParams";
const INVALID_RADAR: &str = "FixtureUnrelatedRadarOption";
const PING: &str = "allowFPSPingWave";

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Complete,
    Invalid,
    HistoricalRadar,
    WrongRadarType,
    WrongRadarStorage,
    WrongPingStorage,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let rules_field = match layout {
        Layout::Complete
        | Layout::HistoricalRadar
        | Layout::WrongRadarType
        | Layout::WrongRadarStorage
        | Layout::WrongPingStorage => TestType::Class(RULES.to_owned()),
        Layout::Invalid => TestType::String,
    };
    DataForgeBuilder::new()
        .version(8)
        .enumeration(
            MODE_ID,
            [
                "FixtureFirst",
                "FixtureSecond",
                "FixtureOther",
                "FixtureNull",
            ],
        )
        .structure(
            TestStruct::new(MODE)
                .field("id", TestType::Enum(MODE_ID.to_owned()))
                .field("params", TestType::StrongPointer(PARAMS.to_owned()))
                .field("fixtureUnrelatedModeValue", TestType::Single),
        )
        .structure(TestStruct::new(PARAMS))
        .structure(
            TestStruct::new(STANDARD)
                .parent(PARAMS)
                .field("fixtureUnrelatedParameter", TestType::Single)
                .field("resourceNetworkParams", rules_field)
                .field("radarOptionParams", radar_field(layout)),
        )
        .structure(TestStruct::new(OTHER_PARAMS).parent(PARAMS))
        .structure(rules_schema())
        .structure(radar_schema(layout))
        .structure(TestStruct::new(INVALID_RADAR).field("unknownFlag", TestType::Bool))
        .record(mode(FIRST, "FixtureFirst", standard(layout, true)))
        .record(mode(SECOND, "FixtureSecond", standard(layout, false)))
        .record(mode(
            OTHER,
            "FixtureOther",
            TestValue::Struct(TestInstance::new(OTHER_PARAMS)),
        ))
        .record(mode(NULL, "FixtureNull", TestValue::Null))
}

fn rules_schema() -> TestStruct {
    [
        "enableItemResourceNetwork",
        "enableSalvageItemRepair",
        "enableMisfires",
        "enableCriticalMisfires",
        "doorsIgnoreRN",
        "enableExperimentalArmorPenetration",
        "shipPassDestroyedPartDamageToComponent",
        "shipPassDestroyedPartDamageToChild",
    ]
    .into_iter()
    .fold(TestStruct::new(RULES), |structure, field| {
        structure.field(field, TestType::Bool)
    })
    .field("shipDestructionBufferMultiplier", TestType::Single)
    .field("shipDestructionHealthThreshold", TestType::Single)
    .field("fixtureRawRule", TestType::Double)
}

fn mode(guid: u128, name: &str, params: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        MODE,
        format!("GameMode.{name}"),
        format!("libs/foundry/records/game_modes/{name}.xml"),
    )
    .value("id", TestValue::text(name))
    .value("params", params)
    .value(
        "fixtureUnrelatedModeValue",
        TestValue::Single(f32::INFINITY),
    )
}

fn standard(layout: Layout, enabled: bool) -> TestValue {
    let network = match layout {
        Layout::Complete
        | Layout::HistoricalRadar
        | Layout::WrongRadarType
        | Layout::WrongRadarStorage
        | Layout::WrongPingStorage => TestValue::Struct(rules(enabled)),
        Layout::Invalid => TestValue::text("fixture invalid resource rule"),
    };
    TestValue::Struct(
        TestInstance::new(STANDARD)
            .value(
                "fixtureUnrelatedParameter",
                TestValue::Single(f32::NEG_INFINITY),
            )
            .value("resourceNetworkParams", network)
            .value("radarOptionParams", radar(layout, enabled)),
    )
}

fn rules(enabled: bool) -> TestInstance {
    TestInstance::new(RULES)
        .value("enableItemResourceNetwork", TestValue::Bool(enabled))
        .value("enableSalvageItemRepair", TestValue::Bool(false))
        .value("enableMisfires", TestValue::Bool(true))
        .value("enableCriticalMisfires", TestValue::Bool(false))
        .value("doorsIgnoreRN", TestValue::Bool(true))
        .value("enableExperimentalArmorPenetration", TestValue::Bool(false))
        .value(
            "shipPassDestroyedPartDamageToComponent",
            TestValue::Bool(enabled),
        )
        .value(
            "shipPassDestroyedPartDamageToChild",
            TestValue::Bool(!enabled),
        )
        .value("shipDestructionBufferMultiplier", TestValue::Single(-0.0))
        .value(
            "shipDestructionHealthThreshold",
            TestValue::Single(f32::from_bits(THRESHOLD_BITS)),
        )
        .value(
            "fixtureRawRule",
            TestValue::Double(f64::from_bits(RAW_BITS)),
        )
}

fn radar_field(layout: Layout) -> TestType {
    match layout {
        Layout::WrongRadarType => TestType::Class(INVALID_RADAR.to_owned()),
        Layout::WrongRadarStorage => TestType::Bool,
        _ => TestType::Class(RADAR.to_owned()),
    }
}

fn radar_schema(layout: Layout) -> TestStruct {
    let radar = [
        "useFPSTagging",
        "useFPSScanMode",
        "enableScanMode",
        "useExperimentalFeatures",
        "allowDeltaSignature",
        "allowEmissionsHUD",
    ]
    .into_iter()
    .fold(TestStruct::new(RADAR), |structure, field| {
        structure.field(field, TestType::Bool)
    })
    .field("fixtureRawRadarOption", TestType::Int32);
    match layout {
        Layout::HistoricalRadar => radar,
        Layout::WrongPingStorage => radar.field(PING, TestType::Int32),
        _ => radar.field(PING, TestType::Bool),
    }
}

fn radar(layout: Layout, enabled: bool) -> TestValue {
    let radar = TestInstance::new(RADAR)
        .value("useFPSTagging", TestValue::Bool(!enabled))
        .value("useFPSScanMode", TestValue::Bool(enabled))
        .value("enableScanMode", TestValue::Bool(true))
        .value("useExperimentalFeatures", TestValue::Bool(false))
        .value("allowDeltaSignature", TestValue::Bool(true))
        .value("allowEmissionsHUD", TestValue::Bool(false))
        .value("fixtureRawRadarOption", TestValue::Int32(17));
    match layout {
        Layout::WrongRadarType => TestValue::Struct(
            TestInstance::new(INVALID_RADAR).value("unknownFlag", TestValue::Bool(enabled)),
        ),
        Layout::WrongRadarStorage => TestValue::Bool(enabled),
        Layout::HistoricalRadar => TestValue::Struct(radar),
        Layout::WrongPingStorage => TestValue::Struct(radar.value(PING, TestValue::Int32(3))),
        _ => TestValue::Struct(radar.value(PING, TestValue::Bool(enabled))),
    }
}
