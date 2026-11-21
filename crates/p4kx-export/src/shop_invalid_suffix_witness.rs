// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch, PathIndex, SourceLocation, SourceUseRole};
use serde::Serialize;

use super::{Component, Containers, Inventory, verdict};
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, with_entry_capture};
use crate::entry_source::EntrySource;
use crate::export_ledger::ExportLedger;
use crate::shop_inventories::ItemList;
use crate::shop_inventory_file::ReadInventory;
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::shop_inventory_path::InventoryDeclaration;
use crate::starmap_index;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
struct Match {
    extension: String,
    authored_directory_entries: usize,
    same_stem_json_entries: usize,
    same_stem_xml_entries: usize,
    owner_conflict: bool,
    outcome: Outcome,
}

#[derive(Clone, Copy, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    Rejected,
    UnprovedSelection,
    Parsed,
    ParsedPartial,
    EmptySource,
    Unreadable,
    Missing,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'data> {
    change: &'data str,
    directory_recovered: bool,
    expected_jsonn: usize,
    parsed_jsonn: usize,
    owner_conflicts: usize,
    matches: &'data [Match],
}

#[test]
#[ignore = "installed Archive, P4KX_SHOP_EXPECT_JSONN_COUNT and a fresh private P4KX_SHOP_SUFFIX_RECEIPT path"]
fn installed_shop_invalid_suffix_targets() -> TestResult {
    let output = std::env::var_os("P4KX_SHOP_SUFFIX_RECEIPT")
        .map(PathBuf::from)
        .ok_or("configure a private suffix receipt path")?;
    let expected: usize = std::env::var("P4KX_SHOP_EXPECT_JSONN_COUNT")
        .map_err(|_| "configure the expected count for the installed Build")?
        .parse()
        .map_err(|_| "invalid expected count")?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive
        .build()
        .ok_or("the installed Archive has no Build")?;
    watch
        .check()
        .map_err(|_| "Archive changed before the comparison")?;
    let matches = with_entry_capture(&archive, |capture| collect(&archive, capture))
        .map_err(|_| "cannot read the original invalid Shop observations")?;
    watch
        .check()
        .map_err(|_| "Archive changed during the comparison")?;
    let parsed = write_receipt(&archive, build.change_number(), &output, expected, &matches)?;
    writeln!(
        io::stdout().lock(),
        "shop_suffix_targets rows={} jsonn_parsed={parsed}",
        matches.len()
    )?;
    let actual = matches
        .iter()
        .filter(|item| item.extension.eq_ignore_ascii_case("jsonn"))
        .count();
    assert_eq!(
        actual, expected,
        "the original structured declarations are retained"
    );
    assert_eq!(
        parsed, expected,
        "every expected replacement passes the ordinary JSON reader"
    );
    Ok(())
}

fn write_receipt(
    archive: &Archive,
    change: &str,
    output: &std::path::Path,
    expected: usize,
    matches: &[Match],
) -> TestResult<usize> {
    let parsed = matches
        .iter()
        .filter(|item| {
            item.extension.eq_ignore_ascii_case("jsonn") && item.outcome == Outcome::Parsed
        })
        .count();
    let owner_conflicts = matches
        .iter()
        .filter(|item| item.extension.eq_ignore_ascii_case("jsonn") && item.owner_conflict)
        .count();
    let output = File::create_new(output).map_err(|_| "create a new private suffix receipt")?;
    serde_json::to_writer(
        output,
        &Receipt {
            change,
            directory_recovered: crate::export_archive_health::inventory(archive).is_some(),
            expected_jsonn: expected,
            parsed_jsonn: parsed,
            owner_conflicts,
            matches,
        },
    )
    .map_err(|_| "write the private suffix receipt")?;
    Ok(parsed)
}

fn collect(archive: &Archive, capture: &EntryCapture<'_>) -> Result<Vec<Match>, CatalogError> {
    let shops = Containers::read(archive, None, capture)?;
    let index = PathIndex::new(archive);
    let mut verifier = Verifier {
        index: &index,
        capture,
        parser: HashMap::new(),
        found: Vec::new(),
    };
    for named in shops.by_entity.values() {
        for entry in &named.observations.rejected {
            if let RejectedInventoryPath::Text(path) = entry {
                verifier.compare(path, Observation::Rejected)?;
            }
        }
        for path in &named.observations.conflict_paths {
            if path.directory_selector.is_some() {
                verifier.compare(
                    &path.original,
                    Observation::Corrected {
                        declaration: path,
                        owner_conflict: true,
                    },
                )?;
            }
        }
        let owner = verdict(&named.inventories);
        for component in &named.inventories {
            let Component::File(stem) = component else {
                continue;
            };
            for path in stem.declarations() {
                if path.directory_selector.is_some() {
                    verifier.compare(
                        &path.original,
                        Observation::Corrected {
                            declaration: path,
                            owner_conflict: owner != Inventory::One(stem.as_str()),
                        },
                    )?;
                }
            }
        }
    }
    verifier.found.sort_unstable();
    Ok(verifier.found)
}

#[derive(Clone, Copy)]
enum Observation<'item> {
    Rejected,
    Corrected {
        declaration: &'item InventoryDeclaration,
        owner_conflict: bool,
    },
}

struct Verifier<'read, 'archive, 'input> {
    index: &'read PathIndex<'archive>,
    capture: &'read EntryCapture<'input>,
    parser: HashMap<String, Outcome>,
    found: Vec<Match>,
}

impl Verifier<'_, '_, '_> {
    fn compare(
        &mut self,
        path: &AuthoredEntryPath,
        observation: Observation<'_>,
    ) -> Result<(), CatalogError> {
        if !is_original_json_path(self.capture, path)? {
            return Ok(());
        }
        let Some(mut directory) = DirectoryMatch::read(self.index, path.as_str()) else {
            return Ok(());
        };
        directory.summary.outcome = match observation {
            Observation::Rejected => Outcome::Rejected,
            Observation::Corrected {
                declaration,
                owner_conflict,
            } => {
                directory.summary.owner_conflict = owner_conflict;
                self.verify(&directory, declaration)?
            }
        };
        self.found.push(directory.summary);
        Ok(())
    }

    fn verify(
        &mut self,
        directory: &DirectoryMatch,
        declaration: &InventoryDeclaration,
    ) -> Result<Outcome, CatalogError> {
        let (summary, expected) = (&directory.summary, directory.stem.as_str());
        if !summary.extension.eq_ignore_ascii_case("jsonn")
            || summary.authored_directory_entries != 0
            || summary.same_stem_json_entries != 1
            || summary.same_stem_xml_entries != 0
            || declaration.stem.as_str() != expected
        {
            return Ok(Outcome::UnprovedSelection);
        }
        let Some(entry) = self.index.find(&directory.target) else {
            return Ok(Outcome::UnprovedSelection);
        };
        let archive = self.index.archive();
        let witnessed = EntrySource::outer(self.capture, archive, entry)?;
        let witnessed = witnessed.receipt().directory_path_selector()?;
        if declaration
            .directory_selector
            .as_ref()
            .is_none_or(|selector| selector.reference() != witnessed.reference())
        {
            return Ok(Outcome::UnprovedSelection);
        }
        if let Some(outcome) = self.parser.get(expected) {
            return Ok(*outcome);
        }
        let mut ledger = ExportLedger::default();
        let read = ReadInventory::read(archive, entry, self.capture, None)?;
        let (_, items, _) = read.retain(&mut ledger);
        let outcome = match items.selected(&mut ledger) {
            ItemList::Items { dropped: 0, .. } => Outcome::Parsed,
            ItemList::Items { .. } => Outcome::ParsedPartial,
            ItemList::Unreadable(_) => Outcome::Unreadable,
            ItemList::EmptySource(_) => Outcome::EmptySource,
            ItemList::Missing => Outcome::Missing,
        };
        self.parser.insert(expected.to_owned(), outcome);
        Ok(outcome)
    }
}

fn is_original_json_path(
    capture: &EntryCapture<'_>,
    path: &AuthoredEntryPath,
) -> Result<bool, CatalogError> {
    let original = path.retain_in(capture)?;
    let (uses, complete) = original.contributions();
    let mut payloads = uses
        .iter()
        .filter(|used| used.role() == SourceUseRole::Payload);
    let Some(payload) = payloads.next().filter(|_| payloads.next().is_none()) else {
        return Ok(false);
    };
    Ok(complete
        && matches!(payload.description().location(),
            SourceLocation::Subresource { namespace, field, indices }
                if (namespace == "socCryXml" || namespace == "socTextXml") && indices.len() == 3
                    && field.eq_ignore_ascii_case("offlineInventoryJSON")
        ))
}

struct DirectoryMatch {
    stem: String,
    target: String,
    summary: Match,
}

impl DirectoryMatch {
    fn read(index: &PathIndex<'_>, original: &str) -> Option<Self> {
        let lookup = starmap_index::key(original);
        let unprefixed = lookup.strip_prefix("data\\").unwrap_or(&lookup);
        let relative = unprefixed.strip_prefix("scripts\\shopinventories\\")?;
        let (name, suffix) = relative.rsplit_once('.')?;
        if name.is_empty() || name.contains('\\') || suffix.is_empty() {
            return None;
        }
        let extension = original.rsplit_once('.')?.1.to_owned();
        let json = format!("scripts\\shopinventories\\{name}.json");
        let xml = format!("scripts\\shopinventories\\{name}.xml");
        Some(Self {
            stem: name.to_owned(),
            target: format!("data\\{json}"),
            summary: Match {
                extension,
                authored_directory_entries: with_data_alias(index, unprefixed)?,
                same_stem_json_entries: with_data_alias(index, &json)?,
                same_stem_xml_entries: with_data_alias(index, &xml)?,
                owner_conflict: false,
                outcome: Outcome::UnprovedSelection,
            },
        })
    }
}

fn with_data_alias(index: &PathIndex<'_>, path: &str) -> Option<usize> {
    Some(count(index, path)? + count(index, &format!("data\\{path}"))?)
}

fn count(index: &PathIndex<'_>, path: &str) -> Option<usize> {
    index
        .matching_entries_with::<Infallible>(path, |_| Ok(()))
        .ok()?
        .try_fold(0, |total, entry| entry.map(|_| total + 1))
        .ok()
}
