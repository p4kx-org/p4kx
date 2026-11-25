// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::shops::{
    InventoryCandidate, ResolutionStatus, ShopInventoryResolutionSnapshot, ShopResolutionSelection,
    read_shop_inventory_resolutions, read_shop_inventory_resolutions_with_provenance_mode,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::{CASES, PREFAB, REGISTRY, TestResult, key, prefab, registry};
use crate::health_origin_assertions as health;

const TABLE: &str = "/declarations/shopInventoryResolution";

#[test]
fn scoped_rows_match_canonical_shop_outcomes_and_field_origins() -> TestResult {
    let (_folder, archive) = super::archive()?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    assert_eq!(
        built.summary().shops().duplicate,
        1,
        "the fixture contains two valid rows for the same normalized registry key"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let first = health::rows(&catalog, "/shops")?
        .iter()
        .find(|shop| shop["key"]["value"] == key(0))
        .ok_or("first valid registry row")?;
    assert_eq!(
        first["registryName"]["value"], "resolved",
        "XML admission keeps the first valid row"
    );
    let table = health::at(&catalog, TABLE)?;
    let snapshot = read_shop_inventory_resolutions_with_provenance_mode(
        &archive,
        ShopResolutionSelection::All,
        full_proof::MODE,
        None,
    )?;
    let scoped = scoped_json(&snapshot);
    health::closure(&scoped)?;
    assert_eq!(
        snapshot.rows.len(),
        CASES.len(),
        "all admitted registry rows survive"
    );
    assert_eq!(
        scoped["availability"], table["availability"],
        "selection preserves canonical table availability"
    );
    assert_eq!(
        health::with_origins(scoped["rows"].clone(), &scoped)?,
        health::with_origins(table["rows"].clone(), &catalog)?,
        "all rows retain exact canonical values, provenance and order"
    );
    let discovery = snapshot
        .coverage
        .discovery
        .ok_or("actual scoped discovery")?;
    assert!(
        snapshot.coverage.registry_complete,
        "registry rows are fully admitted"
    );
    assert_eq!(
        discovery.inventory_payloads, 2,
        "only the unique configured resolved and unreadable files require a body"
    );
    assert!(
        discovery.inventory_directory.source_set_complete(),
        "directory survey is proven"
    );
    Ok(())
}

#[test]
fn key_scope_keeps_missing_and_unreadable_outcomes_distinct() -> TestResult {
    let (_folder, archive) = super::archive()?;
    for (index, status, payloads) in [
        (3, ResolutionStatus::Missing, 0),
        (4, ResolutionStatus::Unreadable, 1),
    ] {
        let key = key(index);
        let snapshot =
            read_shop_inventory_resolutions(&archive, ShopResolutionSelection::Key(&key), None)?;
        let [row] = snapshot.rows.as_slice() else {
            return Err("one exact registry key".into());
        };
        assert_eq!(
            row.shop_key().value(),
            Some(&key),
            "matching full registry chain"
        );
        assert_eq!(
            row.status().value(),
            Some(&status),
            "the actual failure state is typed"
        );
        let discovery = snapshot
            .coverage
            .discovery
            .ok_or("target discovery attempted")?;
        assert_eq!(
            discovery.inventory_payloads, payloads,
            "scope only attempts this body"
        );
        assert!(
            snapshot.coverage.entries.failed_dependencies > 0,
            "failed dependencies remain visible independently of direct shop compatibility"
        );
    }
    let absent = read_shop_inventory_resolutions(
        &archive,
        ShopResolutionSelection::Key("not-a-registry-key"),
        None,
    )?;
    assert!(
        absent.rows.is_empty(),
        "no unrelated placements are returned"
    );
    assert!(
        absent.coverage.discovery.is_none(),
        "no declarations required for an absent key"
    );
    assert!(
        absent.coverage.registry_complete,
        "the unsuccessful lookup scanned the registry"
    );
    Ok(())
}

#[test]
fn unreadable_declarations_keep_the_selected_scope_incomplete_without_dataforge() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("shop-resolution.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(PREFAB, prefab(None)).encrypted())
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let key = key(0);
    let snapshot =
        read_shop_inventory_resolutions(&archive, ShopResolutionSelection::Key(&key), None)?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("archive-only registry row".into());
    };
    let discovery = snapshot
        .coverage
        .discovery
        .ok_or("global declaration search")?;
    assert!(
        !discovery.declarations_complete,
        "an inaccessible declaration could match this key"
    );
    assert_eq!(
        discovery.inventory_payloads, 0,
        "an unreadable declaration grants no target"
    );
    assert_eq!(
        row.status().value(),
        Some(&ResolutionStatus::Unscanned),
        "absence is unproven"
    );
    assert!(
        !row.status().origin().source_set_complete(),
        "surviving evidence is partial"
    );
    assert!(
        snapshot.coverage.entries.failed_dependencies > 0,
        "actual failure is retained"
    );
    Ok(())
}

pub(super) fn assert_physical_duplicate_scope(
    archive: &Archive,
    catalog: &Value,
    canonical: &Value,
    key: &str,
) -> TestResult {
    let snapshot = read_shop_inventory_resolutions_with_provenance_mode(
        archive,
        ShopResolutionSelection::Key(key),
        full_proof::MODE,
        None,
    )?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("one requested registry key".into());
    };
    assert_eq!(
        row.status().value(),
        Some(&ResolutionStatus::Ambiguous),
        "compatible legacy item selection cannot choose the physical resolution target"
    );
    assert_eq!(
        row.physical_targets().value().map(Vec::len),
        Some(2),
        "both physical identities"
    );
    let candidates = row
        .candidates()
        .value()
        .ok_or("sourced declaration candidates")?;
    let [candidate] = candidates.as_slice() else {
        return Err("one declared inventory path".into());
    };
    assert!(
        matches!(candidate.value(), Some(InventoryCandidate::File { stem, .. })
            if stem.value().is_some_and(|value| value == "inv_commex")),
        "the actual declaration can be inspected through the typed API"
    );
    assert_eq!(
        snapshot
            .coverage
            .discovery
            .as_ref()
            .ok_or("actual search")?
            .inventory_payloads,
        0,
        "duplicate directory entries are ambiguous without reading either body"
    );
    let scoped = scoped_json(&snapshot);
    assert_eq!(
        health::with_origins(serde_json::to_value(row)?, &scoped)?,
        health::with_origins(canonical.clone(), catalog)?,
        "the scoped physical row and its evidence equal the normal producer"
    );
    Ok(())
}

fn scoped_json(snapshot: &ShopInventoryResolutionSnapshot) -> Value {
    json!({
        "availability": snapshot.availability,
        "rows": snapshot.rows,
        "provenance": snapshot.source_inventory,
    })
}
