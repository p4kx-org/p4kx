// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use serde_json::Value;

use super::super::super::super::resource_projection;
use super::super::super::projection_tests::compare_regions;
use super::{Failure, OverrideEntryTarget, Result, Written};

#[test]
fn resource_view_preserves_quality_backfill_and_exact_owned_numeric_checks() -> Result<()> {
    let fixture = Written::new(OverrideEntryTarget::Concrete)?;
    let (view, health) = resource_projection::read(&fixture.archive, &fixture.entry, UNIX_EPOCH)
        .map_err(|error| Failure::source("synthetic-resource-projection", error))?;
    compare_health(&health, &fixture.catalog)?;
    compare_regions(&view, &fixture.catalog);
    let scoped = fixture.verify(&view)?;
    let full = fixture.verify(&fixture.catalog)?;
    let counts = |coverage| {
        serde_json::to_value(coverage).map_err(|error| Failure::source("coverage", error))
    };
    assert_eq!(
        counts(scoped)?,
        counts(full)?,
        "scoped and full source coverage"
    );
    let rows = view
        .pointer("/definitions/commodities/rows")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("scoped-quality-commodity-rows"))?;
    assert!(
        rows.iter().any(|row| {
            row.pointer("/selection/value").and_then(Value::as_str) == Some("resourceFallback")
        }),
        "the quality quantization supplies an independently eligible resource fallback"
    );
    Ok(())
}

fn compare_health(health: &crate::ExportHealth, full: &Value) -> Result<()> {
    let kinds = full
        .pointer("/exportHealth/kinds")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("full-resource-health"))?;
    for kind in resource_projection::COVERED {
        let scoped = health
            .kind(kind)
            .ok_or_else(|| Failure::check("scoped-resource-health"))?;
        let label = serde_json::to_value(kind).map_err(|error| Failure::source("kind", error))?;
        let full = kinds
            .iter()
            .find(|facts| facts.get("kind") == Some(&label))
            .ok_or_else(|| Failure::check("full-resource-kind-health"))?;
        let scoped =
            serde_json::to_value(scoped).map_err(|error| Failure::source("kind-health", error))?;
        for field in [
            "sourceInventoryComplete",
            "fieldInventoryComplete",
            "sourceRecords",
            "writtenRows",
        ] {
            assert_eq!(
                scoped.get(field),
                full.get(field),
                "scoped and full resource inventory for {kind:?}.{field}"
            );
        }
    }
    Ok(())
}
