// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use p4kx_extraction::{Archive, Entry};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_entry_read;
use crate::shop_registry::is_guid;
use crate::starmap_index::key;

/// The inventory files that are designer placeholders: the same few commodities for bars,
/// refineries and showrooms alike, or items no Record has. A shop that names one has no list.
pub(crate) const PLACEHOLDERS: [&str; 2] = ["temp_outpost_shopinventory", "inv_cryastro_dummy"];

pub(crate) fn is_placeholder(stem: &str) -> bool {
    PLACEHOLDERS.contains(&stem)
}

const FOLDER: &str = "data\\scripts\\shopinventories\\";
const MAX_INVENTORY_LEN: u64 = 16 << 20;

/// What reading the inventory file of a shop gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemList {
    /// The item GUIDs of the file, lower case, ordered and without repeats (possibly none), and
    /// how many malformed item/ID values were left out.
    Items { guids: Vec<String>, dropped: usize },
    /// The Archive holds no such file.
    Missing,
    /// The file is too large, empty, cannot be opened or is no inventory JSON; holds the path of
    /// the file and why it cannot be read.
    Unreadable(String),
}

/// The inventory files `Data\Scripts\ShopInventories\<stem>.json` of an Archive.
pub(crate) struct Inventories<'a> {
    archive: &'a Archive,
    by_stem: HashMap<String, &'a Entry>,
}

#[derive(Deserialize)]
struct File {
    #[serde(rename = "Collection")]
    collection: Option<Collection>,
    #[serde(rename = "Inventory")]
    inventory: Option<Vec<Element>>,
}

#[derive(Deserialize)]
struct Collection {
    #[serde(rename = "Inventory")]
    inventory: Vec<Element>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Element {
    Known(Identified),
    Malformed(IgnoredAny),
}

#[derive(Deserialize)]
struct Identified {
    #[serde(rename = "ID")]
    id: Id,
}

#[derive(Deserialize)]
struct Id {
    #[serde(rename = "ID")]
    ids: Vec<StoredId>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredId {
    Text(String),
    Malformed(IgnoredAny),
}

impl<'a> Inventories<'a> {
    /// Indexes the `.json` Entries directly below `Data\Scripts\ShopInventories`.
    pub(crate) fn new(archive: &'a Archive) -> Self {
        let by_stem = archive
            .entries()
            .iter()
            .filter_map(|entry| {
                let raw = entry.path();
                let is_json = raw
                    .len()
                    .checked_sub(".json".len())
                    .and_then(|start| raw.get(start..))
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(".json"));
                if !is_json {
                    return None;
                }
                let path = key(raw);
                let file = path.strip_prefix(FOLDER)?;
                let stem = file.strip_suffix(".json")?;
                (!stem.is_empty() && !stem.contains('\\')).then(|| (stem.to_owned(), entry))
            })
            .collect();
        Self { archive, by_stem }
    }

    /// Returns how many inventory files the Archive holds.
    pub(crate) fn count(&self) -> usize {
        self.by_stem.len()
    }

    /// Reads the item GUIDs of the inventory file with the lower-case stem `stem`. Prices, stock
    /// and rental offerings are not read.
    pub(crate) fn items(&self, stem: &str, ledger: &mut ExportLedger) -> ItemList {
        let Some(entry) = self.by_stem.get(stem) else {
            return missing(stem, ledger);
        };
        let source = EntrySource::outer(entry)
            .operation(EntryOperation::ShopInventory)
            .dependency();
        let unreadable = |reason: &dyn std::fmt::Display| {
            ItemList::Unreadable(format!("{}: {reason}", entry.path()))
        };
        let bytes = match shop_entry_read::read(self.archive, entry, MAX_INVENTORY_LEN) {
            Ok(bytes) => bytes,
            Err(error) => {
                source.diagnosed(
                    ledger,
                    Kind::Shops,
                    SourceDisposition::ReadFailed,
                    error.diagnosis(),
                );
                return unreadable(&error);
            }
        };
        if let Some((guids, dropped)) = parse(&bytes) {
            if dropped > 0 {
                source.diagnosed(
                    ledger,
                    Kind::Shops,
                    SourceDisposition::ProjectionFailed,
                    shop_entry_read::diagnosis(
                        ExportDiagnosticCode::MalformedSource,
                        "Inventory[].ID.ID[]",
                    ),
                );
            } else {
                source.record(ledger, Kind::Shops, SourceDisposition::Projected);
            }
            ItemList::Items { guids, dropped }
        } else {
            source.diagnosed(
                ledger,
                Kind::Shops,
                SourceDisposition::ProjectionFailed,
                shop_entry_read::diagnosis(
                    ExportDiagnosticCode::MalformedSource,
                    "Collection.Inventory",
                ),
            );
            unreadable(&why_unparsed(&bytes))
        }
    }
}

fn missing(stem: &str, ledger: &mut ExportLedger) -> ItemList {
    EntrySource::missing(&format!("{FOLDER}{stem}.json"))
        .operation(EntryOperation::ShopInventory)
        .dependency()
        .diagnosed(
            ledger,
            Kind::Shops,
            SourceDisposition::ReadFailed,
            shop_entry_read::diagnosis(ExportDiagnosticCode::DanglingReference, "offlineInventory"),
        );
    ItemList::Missing
}

/// Returns why `bytes`, which [`parse`] rejects, are no inventory.
fn why_unparsed(bytes: &[u8]) -> &'static str {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        "the file is empty"
    } else {
        "the file is no inventory JSON"
    }
}

fn parse(bytes: &[u8]) -> Option<(Vec<String>, usize)> {
    let file: File = serde_json::from_slice(bytes).ok()?;
    let elements = match (file.collection, file.inventory) {
        (Some(collection), _) => collection.inventory,
        (None, Some(inventory)) => inventory,
        (None, None) => return None,
    };
    let mut items = BTreeSet::new();
    let mut dropped = 0;
    for element in elements {
        let Element::Known(element) = element else {
            dropped += 1;
            continue;
        };
        for id in element.id.ids {
            match id {
                StoredId::Text(id) if is_guid(&id) => {
                    items.insert(id.to_ascii_lowercase());
                }
                StoredId::Text(_) | StoredId::Malformed(_) => {
                    dropped += 1;
                }
            }
        }
    }
    Some((items.into_iter().collect(), dropped))
}

#[cfg(test)]
mod tests {
    use super::{parse, why_unparsed};

    #[test]
    fn an_empty_file_is_told_from_one_that_is_no_inventory() {
        assert_eq!(why_unparsed(b""), "the file is empty");
        assert_eq!(why_unparsed(b" \r\n"), "the file is empty");
        assert_eq!(why_unparsed(b"not json"), "the file is no inventory JSON");
        assert_eq!(parse(b""), None);
    }

    #[test]
    fn only_the_item_ids_are_read_ordered_and_without_repeats() {
        let json = br#"{"ShopID":"x","Collection":{"Inventory":[
            {"ID":{"ID":["D195EB47-24B8-45C0-A4D0-1771D60DC1A8"]},"BuyPrice":1.5,"RentalOfferings":[]},
            {"ID":{"ID":["70a98c94-9252-4fd3-a673-5a584339e273"]},"BuyPrice":2.0},
            {"ID":{"ID":["d195eb47-24b8-45c0-a4d0-1771d60dc1a8"]}},
            {"ID":{"ID":["not a guid"]}}]}}"#;
        assert_eq!(
            parse(json),
            Some((
                vec![
                    "70a98c94-9252-4fd3-a673-5a584339e273".to_owned(),
                    "d195eb47-24b8-45c0-a4d0-1771d60dc1a8".to_owned(),
                ],
                1
            )),
            "one value that is no GUID is counted as dropped"
        );
    }

    #[test]
    fn an_empty_inventory_is_an_empty_list_and_other_json_is_unreadable() {
        assert_eq!(
            parse(br#"{"Collection":{"Inventory":[]}}"#),
            Some((Vec::new(), 0))
        );
        assert_eq!(
            parse(br#"{"Inventory":[{"ID":{"ID":["70a98c94-9252-4fd3-a673-5a584339e273"]}}]}"#),
            Some((vec!["70a98c94-9252-4fd3-a673-5a584339e273".to_owned()], 0)),
            "the shape without a Collection"
        );
        assert_eq!(parse(br#"{"Other":1}"#), None);
        assert_eq!(parse(b"not json"), None);
    }
}
