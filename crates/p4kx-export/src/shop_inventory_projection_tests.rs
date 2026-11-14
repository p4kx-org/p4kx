// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::catalog_read::original::with_entry_capture;
use crate::export_ledger::ExportLedger;
use crate::shop_inventory_compatibility::{ItemList, why_unparsed};
use crate::shop_inventory_file::ReadInventory;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn parse(bytes: &[u8]) -> TestResult<Option<(Vec<String>, usize)>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("shop.json", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive.entries().first().ok_or("actual source Entry")?;
    let items = with_entry_capture(&archive, |capture| {
        let read = ReadInventory::read(&archive, entry, capture, None)?;
        Ok(read.items.selected(&mut ExportLedger::default()))
    })?;
    Ok(match items {
        ItemList::Items { guids, dropped } => Some((guids, dropped)),
        ItemList::Missing | ItemList::Unreadable(_) => None,
    })
}

#[test]
fn an_empty_file_is_told_from_one_that_is_no_inventory() -> TestResult {
    assert_eq!(why_unparsed(b""), "the file is empty", "empty bytes");
    assert_eq!(
        why_unparsed(b" \r\n"),
        "the file is empty",
        "only whitespace"
    );
    assert_eq!(
        why_unparsed(b"not json"),
        "the file is no inventory JSON",
        "invalid syntax"
    );
    assert_eq!(
        parse(b"")?,
        None,
        "an empty file has no compatibility items"
    );
    Ok(())
}

#[test]
fn only_the_item_ids_are_read_ordered_and_without_repeats() -> TestResult {
    let json = br#"{"ShopID":"x","Collection":{"Inventory":[
        {"ID":{"ID":["D195EB47-24B8-45C0-A4D0-1771D60DC1A8"]},"BuyPrice":1.5,"RentalOfferings":[]},
        {"ID":{"ID":["70a98c94-9252-4fd3-a673-5a584339e273"]},"BuyPrice":2.0},
        {"ID":{"ID":["d195eb47-24b8-45c0-a4d0-1771d60dc1a8"]}},
        {"ID":{"ID":["not a guid"]}}]}}"#;
    assert_eq!(
        parse(json)?,
        Some((
            vec![
                "70a98c94-9252-4fd3-a673-5a584339e273".to_owned(),
                "d195eb47-24b8-45c0-a4d0-1771d60dc1a8".to_owned(),
            ],
            1
        )),
        "one value that is no GUID is counted as dropped"
    );
    Ok(())
}

#[test]
fn an_empty_inventory_is_an_empty_list_and_other_json_is_unreadable() -> TestResult {
    assert_eq!(
        parse(br#"{"Collection":{"Inventory":[]}}"#)?,
        Some((Vec::new(), 0)),
        "a present empty inventory"
    );
    assert_eq!(
        parse(br#"{"Inventory":[{"ID":{"ID":["70a98c94-9252-4fd3-a673-5a584339e273"]}}]}"#)?,
        Some((vec!["70a98c94-9252-4fd3-a673-5a584339e273".to_owned()], 0)),
        "the shape without a Collection"
    );
    assert_eq!(parse(br#"{"Other":1}"#)?, None, "no inventory shape");
    assert_eq!(parse(b"not json")?, None, "invalid JSON");
    Ok(())
}
