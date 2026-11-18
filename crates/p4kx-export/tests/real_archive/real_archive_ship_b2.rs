// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope, Record,
    SourceEncryption, SourceInput, SourceUses,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{Trace, indexed, rows};
use crate::real_archive_shop_inventory_v3::verify_build;

#[path = "real_archive_ship_b2_values.rs"]
mod values;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_CATALOG_SHIP_SOURCE_RECORD"]
fn installed_ship_b2_matches_original_headers_and_authored_fields() -> TestResult {
    let path = real_archive_path().map_err(|_| "configure the private Archive")?;
    let key = real_archive_key().map_err(|_| "configure the Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry
        .open()
        .map_err(|_| "parse the original DataForge Entry")?;
    let (record, filter) = select(&forge)?;
    let tree = record
        .tree()
        .map_err(|_| "read the selected original Ship body")?;
    let catalog = export(&archive, &entry, &filter)?;
    let ships = rows(&catalog, "/ships")?;
    let [ship] = ships else {
        return Err("the filter must produce exactly one Ship row".into());
    };
    let uses = indexed(&catalog, "/provenance/uses", "id")?;
    let sources = indexed(&catalog, "/provenance/sources", "id")?;
    let origin = entry.origin();
    let DataForgeOrigin::Archive(original) = origin else {
        return Err("a physically observed DataForge Archive Entry is required".into());
    };
    let encryption = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let read_sources = SourceUses::new();
    let scope = read_sources
        .observe(SourceInput::Archive(original), InputScope::Payload)
        .map_err(|_| "bind physical original DataForge payload")?;
    assert!(
        sources.contains_key(scope.source_ref().as_str()),
        "the exact physical DataForge input appears in the published source inventory"
    );
    let trace = Trace::of(&tree, &uses, &scope, encryption);
    values::header(&tree, ship, &trace)?;
    values::body(&tree, ship, &trace)?;
    let manifest_count = values::crew_manifest(&forge, &tree, ship, &trace)?;
    watch
        .check()
        .map_err(|_| "the installed Archive changed during the Ship witness")?;
    crate::real_archive_verification::print_line(&format!(
        "Ship source witness: one physical owner, eight header fields, three vehicle and size fields, six insurance leaf fields, {manifest_count} resolved authored CrewManifest links"
    ));
    Ok(())
}

fn select<'source>(forge: &'source DataForge<'source>) -> TestResult<(Record<'source>, Filter)> {
    let path = std::env::var("P4KX_CATALOG_SHIP_SOURCE_RECORD")
        .map_err(|_| "set a private exact Ship Record path")?;
    if path.is_empty() || path.contains(['*', '?']) {
        return Err("the Ship witness requires a nonempty literal Record path".into());
    }
    let mut exact = forge.records_at_path(&path);
    let record = exact.next().ok_or("selected original Ship Record")?;
    if exact.next().is_some() {
        return Err("the exact selector has multiple physical Ship Records".into());
    }
    let record_path = record
        .record_path()
        .map_err(|_| "read original Ship path")?;
    let record_type = record.type_name().map_err(|_| "read original Ship type")?;
    let name = record.name().map_err(|_| "read original Ship name")?;
    let rule = Kind::Ships.rule().ok_or("canonical Ship selection rule")?;
    if record_type != rule.type_name
        || !rule.matches_path(record_path)
        || is_kind_leftover(Kind::Ships, record_path, name)
    {
        return Err("the nominated physical Record is outside canonical Ship scope".into());
    }
    let filter = Filter::parse(&format!("*{record_path}"))
        .map_err(|_| "construct the Ship Record filter")?;
    let matches = forge
        .records()
        .filter(|candidate| {
            candidate
                .record_path()
                .is_ok_and(|value| filter.matches(value))
        })
        .count();
    if matches != 1 {
        return Err("the catalog Filter must admit one physical Record".into());
    }
    Ok((record, filter))
}

fn export(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build the scoped diagnostic Catalog")?;
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "decode Ship Catalog output")?;
    verify_build(archive, &catalog)?;
    assert!(
        matches!(
            catalog.pointer("/source/channel").and_then(Value::as_str),
            Some("LIVE" | "PTU")
        ),
        "the witness requires an installed LIVE or PTU Build"
    );
    Ok(catalog)
}
