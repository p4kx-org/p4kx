// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use super::{Coverage, Failure, Result};

#[test]
fn bounded_samples_cannot_claim_unobserved_capabilities() {
    let mut coverage = Coverage::default();
    assert!(
        coverage.require_capabilities().is_err(),
        "empty samples are not proof"
    );
    coverage.packaging_slots = 1;
    coverage.volatility_properties = 1;
    assert!(
        coverage.require_capabilities().is_err(),
        "quality must actually be sampled"
    );
    coverage.quality_properties = 1;
    assert!(
        coverage.require_capabilities().is_err(),
        "an empty crafting tag does not prove a compared quality value"
    );
    coverage.quality_values = 1;
    assert!(
        coverage.require_capabilities().is_ok(),
        "all required families were observed"
    );
}

#[test]
fn resource_report_contains_only_anonymized_aggregates() -> Result<()> {
    let coverage = Coverage {
        sampled_rows: 2,
        packaging_slots: 3,
        unresolved_boxes: BTreeMap::from([("dangling", 1)]),
        ..Coverage::default()
    };
    let value = serde_json::to_value(coverage)
        .map_err(|error| Failure::source("synthetic-report", error))?;
    for (name, value) in value
        .as_object()
        .ok_or_else(|| Failure::check("synthetic-report-object"))?
    {
        if name == "unresolvedBoxes" {
            assert_eq!(
                value,
                &serde_json::json!({"dangling": 1}),
                "only fixed reasons and counts"
            );
        } else {
            assert!(value.is_u64(), "coverage fields are counts");
        }
    }
    Ok(())
}
