// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const GLOBAL: &str = "ItemResourceNetworkGlobal";
const DEFAULTS: &str = "ItemResourceNetworkDefaultPowerDistributionParams";
const POWER: &str = "ItemResourceNetworkPowerParams";
const MODIFIER: &str = "ItemResourceNetworkPowerModifier";

#[test]
fn original_global_power_settings_keep_sources_and_authored_extensions() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let marker = input.marker();
        let fixture = Fixture::open(&database(), input, Vec::new())?;
        let (_, catalog) = fixture.diagnostic()?;
        let [row] = origin::rows(&catalog, "/definitions/resourceNetworkGlobals/rows")? else {
            return Err("one physical global resource-network Record".into());
        };
        origin::present(row, "sourceType", &json!(GLOBAL), marker)?;
        let distribution = origin::cell(row, "defaultPowerDistributionParams", "present")?;
        let settings = origin::at(distribution, "/value")?;
        origin::present(settings, "powerDistPoolsFillMin", &json!(false), marker)?;
        let cooler_minimum = origin::cell(settings, "powerDistCoolerMin", "present")?;
        assert_eq!(
            origin::at(cooler_minimum, "/value/rawBits")?,
            "80000000",
            "the authored minimum retains signed zero"
        );
        assert_eq!(
            cooler_minimum["containsEncryptedSource"],
            json!(marker),
            "the minimum uses the original source context"
        );
        assert_ne!(
            cooler_minimum["sourceUsesRefs"], distribution["sourceUsesRefs"],
            "the body and its minimum field own separate source coordinates"
        );
        assert_eq!(
            origin::at(settings, "/additionalFields/roots/0/name/value")?,
            "futureControl",
            "the distribution arena keeps the original extension"
        );
        assert_eq!(
            origin::at(row, "/additionalFields/roots/0/name/value")?,
            "futureDefinition",
            "the root arena keeps the original extension"
        );
        check_modifiers(row, marker)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_modifiers(row: &Value, marker: Option<bool>) -> TestResult {
    let power = origin::at(origin::cell(row, "powerParams", "present")?, "/value")?;
    let [first, second] = origin::rows(power, "/modifiers/value")? else {
        return Err("two original modifier positions".into());
    };
    let first = origin::at(first, "/value")?;
    let second = origin::at(second, "/value")?;
    let left = origin::present(first, "additiveModifier", &json!(-2), marker)?;
    let right = origin::present(second, "additiveModifier", &json!(0), marker)?;
    assert_ne!(
        left["sourceUsesRefs"], right["sourceUsesRefs"],
        "each modifier slot owns its own source coordinates"
    );
    for item in [first, second] {
        origin::present(item, "sourceType", &json!(MODIFIER), marker)?;
        assert_eq!(
            origin::at(item, "/additionalFields/roots/0/name/value")?,
            "futureModifier",
            "the original modifier extension survives"
        );
    }
    Ok(())
}

fn database() -> Vec<u8> {
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
