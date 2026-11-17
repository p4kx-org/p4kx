// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch, PathIndex, SourceLocation, SourceUseRole};
use serde::Serialize;

use super::Containers;
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, with_entry_capture};
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::starmap_index;

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
struct Match {
    extension: String,
    authored_directory_entries: usize,
    same_stem_json_entries: usize,
    same_stem_xml_entries: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'data> {
    change: &'data str,
    directory_recovered: bool,
    matches: &'data [Match],
}

#[test]
#[ignore = "installed Archive and a fresh private P4KX_SHOP_SUFFIX_RECEIPT output path"]
fn installed_shop_invalid_suffix_targets() -> TestResult {
    let output = std::env::var_os("P4KX_SHOP_SUFFIX_RECEIPT")
        .map(PathBuf::from)
        .ok_or("configure a private suffix receipt path")?;
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
    if matches.is_empty() {
        return Err(
            "no eligible invalid inventory attributes have complete source evidence".into(),
        );
    }
    let output = File::create_new(output).map_err(|_| "create a new private suffix receipt")?;
    let directory_recovered = crate::export_archive_health::inventory(&archive).is_some();
    serde_json::to_writer(
        output,
        &Receipt {
            change: build.change_number(),
            directory_recovered,
            matches: &matches,
        },
    )
    .map_err(|_| "write the private suffix receipt")?;
    writeln!(
        io::stdout().lock(),
        "shop_suffix_targets rows={} directory_recovered={directory_recovered}",
        matches.len()
    )?;
    Ok(())
}

fn collect(archive: &Archive, capture: &EntryCapture<'_>) -> Result<Vec<Match>, CatalogError> {
    let shops = Containers::read(archive, None, capture)?;
    let index = PathIndex::new(archive);
    let mut found = Vec::new();
    for path in authored_rejections(&shops) {
        let original = path.retain_in(capture)?;
        let (uses, complete) = original.contributions();
        let mut payloads = uses
            .iter()
            .filter(|used| used.role() == SourceUseRole::Payload);
        let Some(payload) = payloads.next().filter(|_| payloads.next().is_none()) else {
            continue;
        };
        if !complete
            || !matches!(payload.description().location(),
                SourceLocation::Subresource { namespace, field, indices }
                    if namespace == "socCryXml" && indices.len() == 3
                        && field.eq_ignore_ascii_case("offlineInventoryJSON")
            )
        {
            continue;
        }
        if let Some(item) = find_matches(&index, path.as_str()) {
            found.push(item);
        }
    }
    found.sort_unstable();
    Ok(found)
}

fn authored_rejections(shops: &Containers) -> impl Iterator<Item = &AuthoredEntryPath> {
    shops.by_entity.values().flat_map(|named| {
        named
            .observations
            .rejected
            .iter()
            .filter_map(|entry| match entry {
                RejectedInventoryPath::Text(path) => Some(path),
                RejectedInventoryPath::Unreadable(_) => None,
            })
    })
}

fn find_matches(index: &PathIndex<'_>, original: &str) -> Option<Match> {
    let lookup = starmap_index::key(original);
    let unprefixed = lookup.strip_prefix("data\\").unwrap_or(&lookup);
    let relative = unprefixed.strip_prefix("scripts\\shopinventories\\")?;
    let (name, suffix) = relative.rsplit_once('.')?;
    if name.is_empty() || name.contains('\\') || suffix.is_empty() {
        return None;
    }
    let extension = original.rsplit_once('.')?.1.to_owned();
    let authored = format!("data\\{unprefixed}");
    let json = format!("data\\scripts\\shopinventories\\{name}.json");
    let xml = format!("data\\scripts\\shopinventories\\{name}.xml");
    Some(Match {
        extension,
        authored_directory_entries: count(index, &authored)? + count(index, unprefixed)?,
        same_stem_json_entries: count(index, &json)?,
        same_stem_xml_entries: count(index, &xml)?,
    })
}

fn count(index: &PathIndex<'_>, path: &str) -> Option<usize> {
    index
        .matching_entries_with::<Infallible>(path, |_| Ok(()))
        .ok()?
        .try_fold(0, |total, entry| entry.map(|_| total + 1))
        .ok()
}
