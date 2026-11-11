// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::Language;
use serde_json::{Value, json};

use crate::catalog_snapshot_inventory;
use crate::catalog_support::{self, origins};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn successful_machine_summary_keeps_its_exact_catalog_inventory_and_resolves_own_refs() -> TestResult
{
    let dir = tempfile::tempdir()?;
    let archive = catalog_support::write_archive(dir.path())?;
    origins::assert_input_encryption(&archive, Language::English)?;
    let out = dir.path().join("snapshot.json");
    let output = p4kx_testkit::p4kx()?
        .arg("catalog")
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .env("SOURCE_DATE_EPOCH", catalog_support::EXPORTED_AT)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "actual CLI success: {output:?}"
    );
    let summary: Value = serde_json::from_slice(&output.stdout)?;
    let catalog: Value = serde_json::from_slice(&fs::read(&out)?)?;
    assert_summary(&summary, &catalog)?;
    Ok(())
}

fn rejects_unclosed_actual_cell(catalog: &Value, inventory: &Value) -> TestResult {
    let actual = catalog
        .pointer("/definitions/manufacturers/rows/0/guid")
        .ok_or("actual published manufacturer GUID carrier")?;
    assert!(
        actual
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|references| !references.is_empty()),
        "actual GUID contributes"
    );
    let mut changed = actual.clone();
    *changed
        .get_mut("sourceUsesRefs")
        .ok_or("actual GUID use refs")? = json!(["missing-api-test-use"]);
    let error = catalog_snapshot_inventory::check(&changed, inventory)
        .err()
        .ok_or("the own-inventory oracle must reject the dangling use")?;
    assert_eq!(
        error.to_string(),
        "datum reference outside own snapshot",
        "exact closure failure"
    );
    Ok(())
}

fn assert_summary(summary: &Value, catalog: &Value) -> TestResult {
    assert_eq!(summary.get("schema_version"), Some(&json!(3)), "v3 summary");
    let inventory = summary.get("provenance").ok_or("summary snapshot")?;
    assert_eq!(
        catalog.get("provenance"),
        Some(inventory),
        "one actual result inventory"
    );
    assert_eq!(
        summary.get("export_health"),
        catalog.get("exportHealth"),
        "paired health"
    );
    assert!(
        catalog_snapshot_inventory::check(catalog, inventory)? > 0,
        "the actual published sourced values have nonvacuous own-inventory closure"
    );
    catalog_snapshot_inventory::check(summary, inventory)?;
    rejects_unclosed_actual_cell(catalog, inventory)?;
    Ok(())
}
