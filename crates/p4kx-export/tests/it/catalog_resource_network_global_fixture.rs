// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const GLOBAL: &str = "ItemResourceNetworkGlobal";
const DEFAULTS: &str = "ItemResourceNetworkDefaultPowerDistributionParams";
const POWER: &str = "ItemResourceNetworkPowerParams";
pub(super) const MODIFIER: &str = "ItemResourceNetworkPowerModifier";

pub(super) fn database() -> Vec<u8> {
    let defaults = TestStruct::new(DEFAULTS)
        .field("powerDistPoolsFillMin", TestType::Bool)
        .field("powerDistCoolerMin", TestType::Single)
        .field("powerDistPoolsProportion", TestType::Single)
        .field("poolDefaultWeapons", TestType::Single)
        .field("poolDefaultEngines", TestType::Single)
        .field("poolDefaultShields", TestType::Single)
        .field("futureControl", TestType::Single);
    let modifier = TestStruct::new(MODIFIER)
        .field("additiveModifier", TestType::Int32)
        .field("futureModifier", TestType::UInt32);
    let power = TestStruct::new(POWER).array("modifiers", class(MODIFIER));
    let globals = TestStruct::new(GLOBAL)
        .field("powerParams", class(POWER))
        .field("defaultPowerDistributionParams", class(DEFAULTS))
        .field("maxPowerToCoolantRatio", TestType::Single)
        .field("futureDefinition", TestType::Double);
    DataForgeBuilder::new()
        .version(8)
        .structure(defaults)
        .structure(modifier)
        .structure(power)
        .structure(globals)
        .record(record())
        .build()
}

fn record() -> TestRecord {
    let distribution = TestInstance::new(DEFAULTS)
        .value("powerDistPoolsFillMin", TestValue::Bool(false))
        .value("powerDistCoolerMin", TestValue::Single(-0.0))
        .value("powerDistPoolsProportion", TestValue::Single(0.25))
        .value("poolDefaultWeapons", TestValue::Single(0.5))
        .value("poolDefaultEngines", TestValue::Single(0.75))
        .value("poolDefaultShields", TestValue::Single(1.0))
        .value("futureControl", TestValue::Single(1.25));
    let power = TestInstance::new(POWER).value(
        "modifiers",
        TestValue::Array(vec![modifier(-2), modifier(0)]),
    );
    TestRecord::new(
        0x1145,
        GLOBAL,
        "GlobalResources.Test",
        "synthetic/elsewhere/global.xml",
    )
    .value("powerParams", TestValue::Struct(power))
    .value(
        "defaultPowerDistributionParams",
        TestValue::Struct(distribution),
    )
    .value("maxPowerToCoolantRatio", TestValue::Single(0.75))
    .value("futureDefinition", TestValue::Double(1.000_000_000_25))
}

fn modifier(amount: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(MODIFIER)
            .value("additiveModifier", TestValue::Int32(amount))
            .value("futureModifier", TestValue::UInt32(17)),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
