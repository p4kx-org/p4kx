// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult};
use crate::commodity_v3_origin_limit_fixture::{self as limit_fixture, COST_COUNT};

/// Checks payload health independently of origin capture completeness.
pub(crate) fn assert_complete(catalog: &Value) -> TestResult {
    assert_eq!(
        fixture::at(catalog, "/schemaVersion")?,
        &json!(3),
        "the producer publishes the v3 contract"
    );
    assert!(
        catalog.get("commodities").is_none(),
        "no legacy Commodity wire view is emitted"
    );
    let health = fixture::at(catalog, "/exportHealth")?;
    for field in ["status", "read", "projection"] {
        assert_eq!(
            fixture::at(health, &format!("/{field}"))?,
            &json!("complete"),
            "origin capture is not payload failure: {field}"
        );
    }
    for field in ["failedRecords", "failedEntries", "unresolvedReferences"] {
        assert_eq!(
            fixture::at(health, &format!("/{field}"))?
                .as_array()
                .map(Vec::len),
            Some(0),
            "healthy source data has no {field}"
        );
    }
    assert_no_payload_failures(health)?;
    for kind in fixture::at(health, "/kinds")?
        .as_array()
        .ok_or("kind health")?
    {
        assert_kind_complete(kind)?;
    }
    Ok(())
}

fn assert_no_payload_failures(health: &Value) -> TestResult {
    for diagnostic in fixture::at(health, "/diagnostics")?
        .as_array()
        .ok_or("health diagnostics")?
    {
        assert!(
            !matches!(
                diagnostic.get("code").and_then(Value::as_str),
                Some(
                    "sourceReadFailure"
                        | "projectionFailure"
                        | "fieldWalkLimit"
                        | "originMetadataLimit"
                        | "originTargetInventoryLimit"
                )
            ),
            "the independent capture limit does not become a read/projection diagnostic"
        );
    }
    Ok(())
}

fn assert_kind_complete(kind: &Value) -> TestResult {
    for field in ["read", "projection"] {
        assert_eq!(
            fixture::at(kind, &format!("/{field}"))?,
            &json!("complete"),
            "each kind retains complete {field} health"
        );
    }
    for field in ["sourceInventoryComplete", "fieldInventoryComplete"] {
        assert_eq!(
            fixture::at(kind, &format!("/{field}"))?,
            &json!(true),
            "the actual Record and field inventories fit"
        );
    }
    assert_eq!(
        fixture::at(kind, "/failedDerivedProjections")?,
        &json!(0),
        "metadata capture does not fail a derived projection"
    );
    for field in [
        "omittedAfterReadFailure",
        "omittedAfterProjectionFailure",
        "failedDependencies",
        "failedSelectionCandidates",
    ] {
        assert_eq!(
            fixture::at(kind, &format!("/sourceRecords/{field}"))?,
            &json!(0),
            "no Record DTO was lost: {field}"
        );
    }
    for field in [
        "failedRequiredReads",
        "failedProjections",
        "failedDependencies",
        "failedSelectionCandidates",
    ] {
        assert_eq!(
            fixture::at(kind, &format!("/sourceEntries/{field}"))?,
            &json!(0),
            "the Archive inputs are healthy: {field}"
        );
    }
    Ok(())
}

/// Checks every authored cost and its matching ingredient survives capture limits.
pub(crate) fn assert_full_costs(catalog: &Value) -> TestResult {
    let blueprints = fixture::at(catalog, "/blueprints")?
        .as_array()
        .ok_or("blueprints")?;
    assert_eq!(
        blueprints.len(),
        1,
        "the fixture has one cost-bearing Blueprint"
    );
    let blueprint = blueprints.first().ok_or("cost-bearing blueprint")?;
    assert_eq!(
        fixture::at(blueprint, "/guid")?,
        &json!(fixture::guid(limit_fixture::BLUEPRINT)),
        "the original Blueprint identity is retained"
    );
    let tiers = fixture::at(blueprint, "/tiers")?
        .as_array()
        .ok_or("tiers")?;
    assert_eq!(tiers.len(), 1, "the original single tier is retained");
    let tier = tiers.first().ok_or("cost-bearing tier")?;
    assert!(
        tier.get("mandatory").is_some_and(Value::is_null),
        "the mandatory cost field remains explicitly null"
    );
    let costs = fixture::at(tier, "/optional")?
        .as_array()
        .ok_or("optional costs")?;
    let ingredients = fixture::at(blueprint, "/ingredients")?
        .as_array()
        .ok_or("ingredients")?;
    assert_eq!(
        costs.len(),
        usize::from(COST_COUNT),
        "the capture bound never truncates costs"
    );
    assert_eq!(
        ingredients.len(),
        costs.len(),
        "the flattened projection keeps every cost"
    );
    let resource = fixture::guid(fixture::RESOURCE_FALLBACK);
    for ((cost, ingredient), index) in costs.iter().zip(ingredients).zip(0..COST_COUNT) {
        assert_cost_pair(cost, ingredient, index, &resource)?;
    }
    Ok(())
}

fn assert_cost_pair(cost: &Value, ingredient: &Value, index: u16, resource: &str) -> TestResult {
    assert_eq!(
        fixture::at(cost, "/kind")?,
        &json!("resource"),
        "the authored cost keeps its resource kind"
    );
    for line in [cost, ingredient] {
        assert_eq!(
            fixture::at(line, "/resourceGuid")?.as_str(),
            Some(resource),
            "nested and flattened costs retain the resource GUID"
        );
        assert_eq!(
            fixture::at(line, "/quantityScu")?.as_f64(),
            Some(f64::from(index) + 1.0),
            "distinct healthy source quantities keep physical array order"
        );
        assert_eq!(
            fixture::at(line, "/minQuality")?,
            &json!(0),
            "the source minimum quality stays unchanged"
        );
    }
    assert_eq!(
        fixture::at(cost, "/options")?.as_array().map(Vec::len),
        Some(0),
        "no resource cost options are synthesized"
    );
    assert_eq!(
        fixture::at(cost, "/modifiers")?.as_array().map(Vec::len),
        Some(0),
        "no resource cost modifiers are synthesized"
    );
    Ok(())
}

/// Compares payloads while permitting only the expected origin and publication differences.
pub(crate) fn same_payload(left: &Value, right: &Value, path: &str) -> TestResult {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(left.len(), right.len(), "wire keys differ at {path}");
            for (name, left) in left {
                let right = right.get(name).ok_or("payload field disappeared")?;
                if path.is_empty()
                    && matches!(name.as_str(), "source" | "provenance" | "exportHealth")
                {
                    continue;
                }
                if matches!(name.as_str(), "sourceUsesRefs" | "containsEncryptedSource") {
                    continue;
                }
                same_payload(left, right, &format!("{path}/{name}"))?;
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len(), "rows differ at {path}");
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                same_payload(left, right, &format!("{path}/{index}"))?;
            }
        }
        _ => assert_eq!(left, right, "payload or source state differs at {path}"),
    }
    Ok(())
}
