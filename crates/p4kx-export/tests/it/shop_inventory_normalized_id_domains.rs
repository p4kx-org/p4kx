// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{ITEM_A, ORPHAN, TestResult, file, original, referenced};
use crate::health_origin_assertions as health;

#[test]
fn physical_shop_inventory_scope_is_global_without_registry_or_record_matches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let authored = format!(
        r#"{{"ShopID":"authored shop note","Inventory":[{{"ID":{{"ID":["{ITEM_A}","authored item note"]}}}}]}}"#
    );
    crate::starmap_fixture::builder()
        .entry(TestEntry::stored(ORPHAN, authored.as_bytes()))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    assert_eq!(
        original(&archive, ORPHAN, false)?,
        authored,
        "independent plain orphan source"
    );
    let original: Value = serde_json::from_str(&authored)?;
    assert_eq!(
        health::at(&original, "/ShopID")?,
        &json!("authored shop note"),
        "actual malformed shop ID"
    );
    assert_eq!(
        health::at(&original, "/Inventory/0/ID/ID/1")?,
        &json!("authored item note"),
        "actual malformed item ID"
    );
    assert!(
        archive
            .entries()
            .iter()
            .all(|entry| entry.path() != crate::shops_fixture::REGISTRY),
        "the Archive has no shop registry"
    );
    let filter = Filter::new("*/no-matching-record-path/*");
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = health::rows(&catalog, "/definitions/shopInventories/rows")?;
    assert_eq!(rows.len(), 1, "every actual scoped physical file");
    assert_id_domains(file(rows, ORPHAN)?)?;
    assert!(
        health::rows(&catalog, "/shops")?.is_empty(),
        "no filtered shop projection"
    );
    let kind = built
        .summary()
        .kind(Kind::ShopInventories)
        .ok_or("inventory summary kind")?;
    assert_eq!(kind.count, 1, "separate physical inventory row count");
    assert!(
        kind.complete,
        "the Record filter does not narrow this global family"
    );
    Ok(())
}

fn assert_id_domains(row: &Value) -> TestResult {
    let document = health::at(row, "/document/value")?;
    let shop_id = health::at(document, "/fields/shopId")?;
    let valid_id = health::at(
        document,
        "/fields/inventory/value/0/value/id/value/alternatives/value/0",
    )?;
    let invalid_id = health::at(
        document,
        "/fields/inventory/value/0/value/id/value/alternatives/value/1",
    )?;
    for (cell, domain, original) in [
        (shop_id, "gameText", "authored shop note"),
        (valid_id, "public", ITEM_A),
        (invalid_id, "gameText", "authored item note"),
    ] {
        let node = referenced(document, cell)?;
        assert_eq!(
            health::at(node, "/value/value")?,
            &json!({"type":domain,"text":original}),
            "the exact technical or malformed identifier keeps its correct domain"
        );
        assert_eq!(
            node["containsEncryptedSource"], false,
            "the original payload is plain"
        );
    }
    Ok(())
}
