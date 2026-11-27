// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::shops::{self, ShopInventorySelection};
use p4kx_export::{
    CatalogError, CatalogOptions, ExportDiagnosticCode, ExportDiagnosticStage, Kind, build_catalog,
};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct, other_test_key, test_key,
};

use super::TestResult;

const INVENTORY: &str = "Data/Scripts/ShopInventories/Fixture.json";

#[test]
fn filtered_normal_refusal_distinguishes_missing_and_rejected_inventory_keys() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new()
                .version(8)
                .structure(TestStruct::new("Fixture"))
                .build(),
        ))
        .entry(TestEntry::zstd(INVENTORY, r#"{"Inventory":[]}"#).encrypted())
        .write_to(&path)?;
    let filter = Filter::new("*/contractgenerators/*");
    let options = || CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };

    for (key, reason, code) in [
        (
            None,
            "an Archive key is required",
            ExportDiagnosticCode::ArchiveKeyRequired,
        ),
        (
            Some(other_test_key()),
            "the Archive key was rejected",
            ExportDiagnosticCode::WrongArchiveKey,
        ),
    ] {
        let archive = Archive::open_with_key(&path, key)?;
        let Err(error @ CatalogError::IncompleteExport(_)) = build_catalog(&archive, &options())
        else {
            return Err("the Normal catalog must refuse the selected encrypted file".into());
        };
        let CatalogError::IncompleteExport(refusal) = &error else {
            return Err("the Normal export must return a typed refusal".into());
        };
        let diagnostic = refusal
            .health()
            .diagnostics()
            .iter()
            .find(|row| row.kind == Some(Kind::ShopInventories) && row.code == code)
            .ok_or("the Normal refusal retained the actual inventory key error")?;
        assert_eq!(diagnostic.count, 1, "one blocked physical inventory read");
        assert_eq!(
            diagnostic.stage,
            ExportDiagnosticStage::SourceRead,
            "opening the Entry failed"
        );

        let scoped = shops::read_shop_inventories(
            &archive,
            ShopInventorySelection::ArchivePath(INVENTORY),
            None,
        )?;
        let [row] = scoped.rows.as_slice() else {
            return Err("the selected physical inventory still has a definition row".into());
        };
        let [failure] = scoped.coverage.failures.as_slice() else {
            return Err("exactly one original scoped read failure".into());
        };
        assert_eq!(
            failure.entry_index,
            row.identity().entry_index(),
            "physical inventory identity"
        );
        assert_eq!(
            failure.stage, diagnostic.stage,
            "both routes retain the same stage"
        );
        assert_eq!(
            failure.code, code,
            "both routes retain the exact key failure"
        );
        let message = error.to_string();
        assert!(
            message.contains("definitions.shopInventories") && message.contains(reason),
            "the refusal must name the global dataset and its specific key failure: {message}"
        );
    }

    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let catalog = build_catalog(&archive, &options())?;
    let kind = catalog
        .summary()
        .kind(Kind::ShopInventories)
        .ok_or("the global inventory table")?;
    assert_eq!(
        kind.count, 1,
        "the Record filter keeps the readable inventory"
    );
    assert!(
        catalog.summary().is_complete(),
        "the keyed control publishes normally"
    );
    Ok(())
}
