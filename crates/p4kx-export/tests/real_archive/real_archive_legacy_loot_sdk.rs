// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::loot::LegacyLootSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, Filter};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_auto_spawns::provenance;
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_item_durability::rows;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "real_archive_legacy_loot_source.rs"]
mod source;

use source::{Family, Sample};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_legacy_loot_sdk_matches_each_physical_producer_row() -> TestResult {
    let path = real_archive_path().map_err(|_| "configure the installed Archive path")?;
    let key = real_archive_key().map_err(|_| "configure the installed Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the configured Archive")?;
    let change = std::env::var("P4KX_LEGACY_LOOT_BUILD")
        .map_err(|_| "set the exact installed Loot witness changelist")?;
    if change.is_empty()
        || !change.bytes().all(|byte| byte.is_ascii_digit())
        || archive
            .build()
            .is_none_or(|build| build.change_number() != change)
    {
        return Err("the installed Archive has a different or missing Build".into());
    }
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "parse the original DataForge")?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let results = [Family::Table, Family::Archetype].map(|family| {
        let sample = source::select(&forge, &watch, family)?;
        verify(&archive, &entry, &parsed, family, &sample)
    });
    for result in results {
        result?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    print_line(&format!(
        "Legacy Loot SDK parity complete for Build {change}"
    ));
    Ok(())
}

fn verify(
    archive: &Archive,
    entry: &DataForgeEntry,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    sample: &Sample<'_>,
) -> TestResult {
    let index = sample.tree.record_index();
    let selection = CatalogRecordSelection::RecordIndex(index);
    let sdk = match family {
        Family::Table => parsed.with_legacy_loot_tables(selection, None, |snapshot| {
            capture(archive, index, &snapshot)
        })??,
        Family::Archetype => {
            parsed.with_legacy_loot_archetypes(selection, None, |snapshot| {
                capture(archive, index, &snapshot)
            })??
        }
    };
    let catalog = produce(archive, entry, sample.tree.record_path())?;
    let mut owners = rows(&catalog, family.pointer())?.iter().filter(|row| {
        row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
    });
    let producer = owners
        .next()
        .ok_or("the filtered producer lacks the selected Record")?;
    if owners.next().is_some() || producer != &sdk.row {
        return Err("the exact SDK and producer rows or cited source-use IDs differ".into());
    }
    sdk.origins.verify_in_document(&catalog)?;
    let original_origins = provenance::Origins::read(&catalog, entry.origin())?;
    source::check(family, sample, producer, &original_origins)?;
    print_line(&format!(
        "Legacy Loot {} one physical row matched; source uses={} physical sources={} incomplete SDK fields={} dangling direct Reference checked={}",
        family.label(),
        sdk.uses,
        sdk.sources,
        sdk.incomplete,
        sample.dangling.is_some(),
    ));
    Ok(())
}

fn produce(archive: &Archive, entry: &DataForgeEntry, path: &str) -> TestResult<Value> {
    if path.is_empty() || path.contains(['*', '?']) {
        return Err("the selected Record does not have a literal producer filter".into());
    }
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())
        .map_err(|_| "build the owner-filtered Legacy Loot catalog")?;
    let catalog = serde_json::from_slice(built.json())?;
    verify_build(archive, &catalog)?;
    Ok(catalog)
}

struct Captured {
    row: Value,
    origins: Origins,
    incomplete: bool,
    uses: usize,
    sources: usize,
}

fn capture<Row: Serialize>(
    archive: &Archive,
    index: u32,
    snapshot: &LegacyLootSnapshot<Row>,
) -> TestResult<Captured> {
    if snapshot.build.as_ref() != archive.build() {
        return Err("selective Legacy Loot Build differs from the installed Archive".into());
    }
    if !snapshot.coverage.unreadable_headers.is_empty() {
        return Err("the exact SDK Record header is unreadable".into());
    }
    if !snapshot.coverage.unreadable_bodies.is_empty() {
        return Err("the exact SDK Record body is unreadable".into());
    }
    if !matches!(snapshot.availability, DatasetAvailability::Available) {
        return Err("the exact SDK reader reported the admitted Loot family unavailable".into());
    }
    let [row] = snapshot.rows.as_slice() else {
        return Err("the exact SDK scope must return one original physical Record".into());
    };
    let incomplete = match snapshot.coverage.incomplete_fields.as_slice() {
        [] => false,
        [owner] if *owner == index => true,
        _ => return Err("the exact SDK reported incomplete fields outside its owner".into()),
    };
    let row = wire_row(row)?;
    let mut origins = Origins::default();
    origins.extend(&snapshot.source_inventory, std::slice::from_ref(&row))?;
    Ok(Captured {
        row,
        origins,
        incomplete,
        uses: snapshot.source_inventory.use_count(),
        sources: snapshot.source_inventory.source_count(),
    })
}
