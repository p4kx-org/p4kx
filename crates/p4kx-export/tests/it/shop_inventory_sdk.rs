// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::shops::{
    ShopInventorySelection, ShopInventoryToken, read_shop_inventories,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, CancelToken, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct, test_key};
use serde_json::{Value, json};

use super::TestResult;
use crate::health_origin_assertions as proof;

const NAME: &str = "Data\\Scripts\\ShopInventories\\Inv_Duplicate.json";
const ALIAS: &str = "data/scripts/shopinventories/inv_duplicate.JSON";
const BAD: &str = "Data\\Scripts\\ShopInventories\\Inv_Malformed.json";
const EMPTY: &str = "Data\\Scripts\\ShopInventories\\Inv_ZeroBytes.json";
const KEYED: &str = "Data\\Scripts\\ShopInventories\\Inv_Keyed.json";
const DATASET: &str = "/definitions/shopInventories/rows";
const EXACT_PRICE: &str = "18446744073709551616";

#[test]
fn shop_inventory_sdk_matches_the_canonical_definition_table() -> TestResult {
    let (_directory, archive) = fixture(true)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let result = read_shop_inventories(&archive, ShopInventorySelection::All, None)?;
    assert!(matches!(
        result.availability,
        DatasetAvailability::Available
    ));
    assert_eq!(result.rows.len(), 5);
    assert_eq!(result.coverage.entries.projected, 4);
    assert_eq!(result.coverage.entries.failed_projections, 1);
    let scoped = json!({
        "definitions": {"shopInventories": {"rows": result.rows}},
        "provenance": result.source_inventory,
        "directory": result.coverage.directory,
    });
    assert_eq!(
        proof::rows(&scoped, DATASET)?,
        proof::rows(&catalog, DATASET)?,
        "the canonical rows, duplicate identities and original value evidence agree"
    );
    proof::closure(&scoped)
}

#[test]
fn shop_inventory_sdk_reads_selected_entries_without_dataforge_or_registry() -> TestResult {
    let (_directory, archive) = fixture(false)?;
    let result = read_shop_inventories(&archive, ShopInventorySelection::ArchivePath(ALIAS), None)?;
    assert_eq!(result.coverage.entries.selected, 2);
    assert_eq!(result.coverage.entries.failed_required_reads, 0);
    let [first, second] = result.rows.as_slice() else {
        return Err("both physical duplicates must remain addressable".into());
    };
    assert!(first.identity().entry_index() < second.identity().entry_index());
    assert_ne!(first.identity().input_ref(), second.identity().input_ref());
    let document = first
        .document()
        .value()
        .ok_or("the original JSON document")?;
    let lines = document
        .fields()
        .inventory()
        .value()
        .ok_or("inventory list")?;
    let [line] = lines.as_slice() else {
        return Err("one authored inventory line".into());
    };
    let line = line.value().ok_or("original inventory line")?;
    let price = line
        .buy_price()
        .value()
        .ok_or("original buy-price address")?;
    let cell = document
        .source()
        .values()
        .get(usize::try_from(price.get())?)
        .ok_or("original price cell")?;
    assert!(
        matches!(cell.value(), Some(ShopInventoryToken::Number { token, .. }) if token == EXACT_PRICE),
        "the original number token must retain every digit"
    );
    let chosen = read_shop_inventories(
        &archive,
        ShopInventorySelection::EntryIndex(second.identity().entry_index()),
        None,
    )?;
    assert_eq!(chosen.rows.len(), 1);
    assert_eq!(
        chosen.rows[0].identity().input_ref(),
        second.identity().input_ref()
    );
    let raw =
        json!({"provenance": chosen.source_inventory, "directory": chosen.coverage.directory});
    let uses = proof::uses(&raw, proof::at(&raw, "/directory")?)?;
    assert_eq!(
        uses.len(),
        2,
        "an index lookup cites the selected Entry path and position"
    );
    assert!(
        uses.iter()
            .all(|input| input.pointer("/location/type") == Some(&json!("metadata")))
    );
    Ok(())
}

#[test]
fn shop_inventory_sdk_reports_distinct_empty_failed_and_absent_states() -> TestResult {
    let (_directory, archive) = fixture(false)?;
    for (path, state, projected, read_failures, projection_failures) in [
        (EMPTY, SourceValueState::Absent, 1, 0, 0),
        (KEYED, SourceValueState::Unreadable, 0, 1, 0),
        (BAD, SourceValueState::Invalid, 0, 0, 1),
    ] {
        let selected =
            read_shop_inventories(&archive, ShopInventorySelection::ArchivePath(path), None)?;
        let [row] = selected.rows.as_slice() else {
            return Err("one observed physical Entry".into());
        };
        assert_eq!(row.document().state(), state);
        assert_eq!(selected.coverage.entries.projected, projected);
        assert_eq!(
            selected.coverage.entries.failed_required_reads,
            read_failures
        );
        assert_eq!(
            selected.coverage.entries.failed_projections,
            projection_failures
        );
    }
    let missing =
        read_shop_inventories(&archive, ShopInventorySelection::EntryIndex(u32::MAX), None)?;
    assert!(matches!(missing.availability, DatasetAvailability::Absent));
    assert!(missing.coverage.directory.source_set_complete());
    let raw =
        json!({"provenance": missing.source_inventory, "directory": missing.coverage.directory});
    let uses = proof::uses(&raw, proof::at(&raw, "/directory")?)?;
    let [count_use] = uses.as_slice() else {
        return Err("missing index must cite the admitted directory length alone".into());
    };
    assert_eq!(
        count_use.pointer("/location/field"),
        Some(&json!("entryCount"))
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        read_shop_inventories(&archive, ShopInventorySelection::All, Some(&cancel)),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}

fn fixture(with_dataforge: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("shop-sources.p4k");
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored(
            NAME,
            format!(r#"{{"Inventory":[{{"BuyPrice":{EXACT_PRICE}}}]}}"#),
        ))
        .entry(TestEntry::stored(ALIAS, r#"{"Inventory":[]}"#))
        .entry(TestEntry::stored(BAD, b"{"))
        .entry(TestEntry::stored(EMPTY, Vec::new()))
        .entry(TestEntry::stored(KEYED, r#"{"Inventory":[]}"#).encrypted());
    let builder = if with_dataforge {
        builder.entry(TestEntry::stored(
            "Data\\Game2.dcb",
            DataForgeBuilder::new()
                .version(8)
                .structure(TestStruct::new("Fixture"))
                .build(),
        ))
    } else {
        builder
    };
    builder.write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(&path, with_dataforge.then(test_key))?,
    ))
}
