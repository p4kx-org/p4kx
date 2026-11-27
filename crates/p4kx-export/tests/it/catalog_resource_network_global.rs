// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

#[path = "catalog_resource_network_global_fixture.rs"]
mod fixture;
#[path = "catalog_resource_network_global_sdk.rs"]
mod sdk;

use fixture::{GLOBAL, MODIFIER, database};

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
