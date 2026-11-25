// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::Language;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
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
    let (summary, catalog) = run_catalog(&archive, &out)?;
    assert_summary(&summary, &catalog)?;
    let inventory = summary.get("provenance").ok_or("summary snapshot")?;
    rejects_unclosed_actual_cell(&catalog, inventory)?;
    Ok(())
}

#[test]
fn machine_summary_keeps_shared_item_variant_provenance() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let mut database = entity_schema(DataForgeBuilder::new().version(8));
    for (guid, name) in [(1, "POWR_Parity_A"), (2, "POWR_Parity_B")] {
        let record = TestEntity::new(
            guid,
            name,
            format!("libs/foundry/records/entities/scitem/ships/powerplant/{name}.xml"),
        )
        .attach_type("PowerPlant")
        .size(1)
        .physics_mass(10.0)
        .record();
        database = database.record(record);
    }
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&archive)?;

    let out = dir.path().join("variants.json");
    let (summary, catalog) = run_catalog(&archive, &out)?;
    let sets = catalog
        .pointer("/provenance/variantSourceUseSets")
        .and_then(Value::as_array)
        .ok_or("the written catalog's variant source-use sets")?;
    let [set] = sets.as_slice() else {
        return Err("the two comparable physical Items must share one source-use set".into());
    };
    let id = set.get("id").and_then(Value::as_u64).ok_or("pool ID")?;
    assert!(
        set.get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|refs| !refs.is_empty()),
        "the pool contains actual contributing source uses"
    );
    let items = catalog["items"].as_array().ok_or("the catalog Items")?;
    assert_eq!(items.len(), 2, "both physical Items were written");
    for item in items {
        assert_eq!(
            item.pointer("/variantOf/sourceUseSetRef")
                .and_then(Value::as_u64),
            Some(id),
            "each variant decision reaches the pool in its own result"
        );
    }
    assert_summary(&summary, &catalog)
}

fn run_catalog(archive: &Path, out: &Path) -> Result<(Value, Value), Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["catalog", "--provenance", "full"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .arg("--json")
        .env("SOURCE_DATE_EPOCH", catalog_support::EXPORTED_AT)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "actual CLI success: {output:?}"
    );
    let summary: Value = serde_json::from_slice(&output.stdout)?;
    let catalog: Value = serde_json::from_slice(&fs::read(out)?)?;
    Ok((summary, catalog))
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
    Ok(())
}
