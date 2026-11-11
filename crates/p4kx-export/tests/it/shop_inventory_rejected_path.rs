// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::{Value, json};

use super::{
    INVENTORIES, ITEM_A, PREFAB, REGISTRY, TestResult, key, original, prefab, registry,
    resolution_row, uses,
};
use crate::health_origin_assertions as health;

const REJECTED_PATH: &str = "Scripts/ShopInventories/unsupported.csv";

fn actual_rejected_prefab(rejected: Option<&str>) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(PREFAB, prefab(rejected)))
        .entry(TestEntry::stored(
            format!("{INVENTORIES}Inv_Resolved.json"),
            format!(r#"{{"Inventory":[{{"ID":{{"ID":["{ITEM_A}"]}}}}]}}"#),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let xml = original(&archive, PREFAB)?;
    let prefab = roxmltree::Document::parse(&xml)?;
    let actual = prefab
        .descendants()
        .find_map(|node| node.attribute("offlineInventoryRejected"));
    assert_eq!(actual, rejected, "the actual authored rejected slot");
    Ok((dir, archive))
}

#[test]
fn rejected_populated_path_stays_visible_without_false_completeness() -> TestResult {
    for rejected in [None, Some(REJECTED_PATH)] {
        let (_dir, archive) = actual_rejected_prefab(rejected)?;
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = p4kx_export::build_catalog(&archive, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let rows = health::rows(&catalog, "/declarations/shopInventoryResolution/rows")?;
        let row = resolution_row(rows, &key(0))?;
        assert_eq!(
            health::at(row, "/status/value")?,
            &json!("resolved"),
            "same accepted physical target"
        );
        assert_eq!(
            health::at(row, "/candidates/sourceSetComplete")?,
            &json!(rejected.is_none()),
            "all actual declarations affect the closure"
        );
        let candidates = health::rows(row, "/candidates/value")?;
        if let Some(path) = rejected {
            let candidate = candidates
                .iter()
                .find(|value| {
                    value.pointer("/value/type").and_then(Value::as_str) == Some("rejected")
                })
                .ok_or("the exact rejected candidate")?;
            let path = health::present(
                health::at(candidate, "/value")?,
                "sourcePath",
                &json!(path),
                Some(false),
            )?;
            assert!(
                uses(&catalog, path)?.iter().any(|used| used
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    == Some("@offlineInventoryRejected")),
                "the rejected text is tied to the actual attribute"
            );
        }
    }
    Ok(())
}
