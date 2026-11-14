// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::TestResult;

const CREATURE_DEFAULT: &str = "1dcde125-1275-474f-b0d7-d6682e9e521f";
const PYRO: &str = "286cb603-b4ae-4279-80a1-d4505fee1916";

pub(super) fn check_sources(catalog: &Value) -> TestResult {
    let distributions = super::list(catalog, "qualityDistributions")?;
    let default = distributions
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(CREATURE_DEFAULT))
        .ok_or("the default creature distribution")?;
    let curve = default
        .pointer("/qualityDistribution/value/payload/value/value/value")
        .ok_or("canonical inline quality curve")?;
    assert_eq!(
        default
            .pointer("/qualityDistribution/value/sourceType/value")
            .and_then(Value::as_str),
        Some("CraftingQualityDistributionNormal"),
        "the original concrete distribution type remains explicit"
    );
    assert!(
        curve.get("kind").is_none(),
        "the raw payload has no duplicate concrete-type declaration"
    );
    for (field, expected) in [("min", 1), ("max", 1000)] {
        assert_eq!(
            curve
                .pointer(&format!("/{field}/value/type"))
                .and_then(Value::as_str),
            Some("int32"),
            "the original bound width"
        );
        assert_eq!(
            curve
                .pointer(&format!("/{field}/value/value"))
                .and_then(Value::as_i64),
            Some(expected),
            "the configured bound"
        );
    }
    for (field, expected) in [("mean", 269.0), ("stddev", 254.0)] {
        assert_eq!(
            curve
                .pointer(&format!("/{field}/value/finite"))
                .and_then(Value::as_f64),
            Some(expected),
            "the source parameter"
        );
    }
    let overrides = distributions.iter().filter_map(|row| {
        row.pointer("/locationOverride/value/payload/value/value/value/locationOverrideList/value")
            .and_then(Value::as_array)
    });
    assert!(
        overrides.flatten().any(|entry| entry
            .pointer("/value/payload/value/location/value/guid/value")
            .and_then(Value::as_str)
            == Some(PYRO)
            && entry
                .pointer("/value/payload/value/qualityDistribution/value")
                .is_some_and(Value::is_object)),
        "an original ordered location override for Pyro"
    );
    Ok(())
}
