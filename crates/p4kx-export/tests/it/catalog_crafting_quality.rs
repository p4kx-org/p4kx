// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::crafting_detail_fixture as fixture;
use crate::quality_v3_support as source;

use super::TestResult;

pub(super) fn assert_location_overrides(catalog: &Value) -> TestResult {
    let list = source::rows(catalog, source::DISTRIBUTIONS)?;
    assert_eq!(
        list.len(),
        2,
        "the default distribution and location override retain separate physical rows"
    );
    let normal = list.first().ok_or("default quality")?;
    assert_normal(normal)?;
    let overrides = list.get(1).ok_or("override quality")?;
    assert_override(overrides)
}

fn assert_normal(normal: &Value) -> TestResult {
    assert_eq!(
        source::at(normal, "/guid/value")?,
        &json!(catalog_crafting_support::text(fixture::DEFAULT_CURVE)),
        "the default row retains its original distribution identity"
    );
    let curve = source::at(normal, source::CURVE)?;
    for (field, value) in [("min", 1), ("max", 1000)] {
        assert_eq!(
            source::at(curve, &format!("/{field}/value"))?,
            &json!({"type":"int32", "value":value}),
            "the default distribution keeps the original Int32 {field} bound"
        );
    }
    assert_eq!(
        source::at(normal, "/qualityDistribution/value/sourceType/value")?,
        &json!("CraftingQualityDistributionNormal"),
        "the default wrapper retains its actual concrete source type"
    );
    assert!(
        curve.get("kind").is_none(),
        "the owning wrapper retains the concrete source type exactly once"
    );
    assert_eq!(
        source::at(curve, "/mean/value/finite")?,
        &json!(269.0),
        "the default mean remains distinct from the location override mean"
    );
    assert_eq!(
        source::at(curve, "/stddev/value/finite")?,
        &json!(254.0),
        "the default spread remains distinct from the location override spread"
    );
    source::state(source::at(normal, "/locationOverride")?, "absent")?;
    Ok(())
}

fn assert_override(overrides: &Value) -> TestResult {
    source::state(source::at(overrides, "/qualityDistribution")?, "absent")?;
    let entry = source::at(
        overrides,
        "/locationOverride/value/payload/value/value/value/locationOverrideList/value/0/value/payload/value",
    )?;
    assert_eq!(
        source::at(entry, "/location/value/guid/value")?,
        &json!(catalog_crafting_support::text(fixture::PYRO)),
        "the ordered override entry retains its original location Reference"
    );
    let curve = source::at(
        entry,
        "/qualityDistribution/value/payload/value/value/value",
    )?;
    assert_eq!(
        source::at(curve, "/mean/value/finite")?,
        &json!(295.0),
        "the override retains its configured mean independently of the default"
    );
    assert_eq!(
        source::at(curve, "/stddev/value/finite")?,
        &json!(1.0),
        "the override retains its configured spread independently of the default"
    );
    Ok(())
}
