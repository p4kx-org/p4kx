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
                Some("sourceReadFailure" | "projectionFailure" | "fieldWalkLimit")
            ),
            "complete original relationships do not produce read, projection or field-walk failures"
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

/// Checks every authored cost and matching ingredient after the original input count.
pub(crate) fn assert_full_costs(catalog: &Value) -> TestResult {
    use crate::catalog_crafting_support::blueprint;
    assert_eq!(
        blueprint::definitions(catalog)?.len(),
        1,
        "one cost-bearing Blueprint"
    );
    let blueprint = blueprint::by_guid(catalog, limit_fixture::BLUEPRINT)?;
    assert_eq!(
        fixture::at(blueprint.definition, "/guid/value")?.as_str(),
        Some(fixture::guid(limit_fixture::BLUEPRINT).as_str()),
        "the arena is bound to the authored Blueprint"
    );
    let tiers = blueprint.tiers()?;
    let [tier] = tiers.as_slice() else {
        return Err("one original tier".into());
    };
    assert!(
        tier["mandatory"].is_null(),
        "the mandatory cost remains absent from the recipe"
    );
    let tier_source = blueprint.node(&tier["source"])?;
    let recipe = blueprint.field(tier_source, "recipe")?;
    let cost_source = blueprint.field(recipe, "costs")?;
    let mandatory = blueprint.field(cost_source, "mandatoryCost")?;
    assert_eq!(
        fixture::at(mandatory, "/value/state")?,
        "null",
        "the actual mandatory slot is explicitly null"
    );

    let refs = tier["optional"]
        .as_array()
        .ok_or("optional cost references")?;
    let costs = tier["costs"].as_array().ok_or("flat typed costs")?;
    let ingredients = blueprint.presentation["ingredients"]
        .as_array()
        .ok_or("ingredients")?;
    assert_eq!(
        refs.len(),
        usize::from(COST_COUNT),
        "every original optional slot has a reference"
    );
    assert_eq!(
        costs.len(),
        refs.len(),
        "each optional slot has one typed cost"
    );
    assert_eq!(
        ingredients.len(),
        refs.len(),
        "each optional resource cost has one ingredient"
    );
    let resource = fixture::guid(fixture::RESOURCE_FALLBACK);
    for (index, ((reference, cost), ingredient)) in
        (0..COST_COUNT).zip(refs.iter().zip(costs).zip(ingredients))
    {
        assert_cost(index, reference, cost, ingredient, &resource)?;
    }
    Ok(())
}

fn assert_cost(
    index: u16,
    reference: &Value,
    cost: &Value,
    ingredient: &Value,
    resource: &str,
) -> TestResult {
    assert_eq!(reference, &cost["source"], "physical optional slot order");
    assert_eq!(
        reference, &ingredient["cost"],
        "the ingredient links the same authored cost"
    );
    assert!(
        ingredient["slot"].is_null(),
        "a cost outside a selection has no slot"
    );
    let meaning = &cost["meaning"];
    assert_eq!(
        meaning["type"], "resource",
        "the original cost is typed as a resource"
    );
    assert_eq!(
        fixture::at(meaning, "/resourceGuid/value")?.as_str(),
        Some(resource),
        "the typed cost retains its authored resource link"
    );
    assert_eq!(
        meaning
            .pointer("/quantityScu/value")
            .and_then(Value::as_f64),
        Some(f64::from(index) + 1.0),
        "quantity matches the authored slot position"
    );
    assert_eq!(
        fixture::at(meaning, "/minQuality/value")?,
        "0",
        "minimum quality is preserved"
    );
    assert!(
        meaning.get("options").is_none(),
        "no resource cost options are synthesized"
    );
    assert_eq!(
        cost.pointer("/context/modifiers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "no modifiers are invented"
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
