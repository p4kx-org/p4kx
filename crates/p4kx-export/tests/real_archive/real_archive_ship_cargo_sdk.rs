// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_assembly::{ArchiveFiles, ENTITY_TYPE, EntityNode, LoadoutResolver};
use p4kx_export::catalog_queries::equipment::{
    ShipBaseFacts, ShipCargoSource, ShipSourceSnapshot, with_ship_sources_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, SCHEMA_VERSION, build_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, Record};
use serde_json::Value;

use super::{TestResult, legacy_grid, legacy_total, same_scu};
use crate::real_archive_catalog_table_parity::{Origins, wire_row};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "real_archive_ship_cargo_sdk_receipt.rs"]
mod receipt;

const DATASET: &str = "/projections/shipBaseFacts";

#[test]
#[ignore = "real Archive: configure the exact Build, one physical Record and a new private cargo SDK receipt"]
fn installed_ship_cargo_matches_scoped_producer_and_original_loadout() -> TestResult {
    let target = receipt::destination()?;
    if target.try_exists()? {
        return Err("the cargo SDK witness needs a new private receipt".into());
    }
    let archive = receipt::open_archive()?;
    let stamp = receipt::Stamp::read(&archive)?;
    let watch = ArchiveWatch::new(&archive);
    let entry = read_dataforge(&archive)?;
    let forge = entry
        .open()
        .map_err(|_| "parse the original DataForge Entry")?;
    let index = selected_record_index(&forge)?;
    let (filter, admitted, record) = select(&forge, index)?;
    let catalog = produce(&archive, &entry, &filter)?;
    let mut origins = Origins::default();
    let row = query(&archive, &entry, index, |snapshot, facts| {
        compare_original(&archive, &forge, &record, facts)?;
        let row = wire_row(facts)?;
        origins.extend(snapshot.source_inventory(), std::slice::from_ref(&row))?;
        Ok(row)
    })?;
    compare_catalog(&catalog, &admitted, index, &row)?;
    origins.verify_in_document(&catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during cargo SDK verification")?;
    receipt::Receipt::new(stamp, index, &row)?.write(&target)?;
    print_line(
        "Cargo SDK: one physical Ship matched the scoped normal producer, original Loadout and cited origins",
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set the exact Build and existing private cargo SDK receipt"]
fn installed_ship_cargo_sdk_cold_query_replays_one_verified_root() -> TestResult {
    let archive = receipt::open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let verified = receipt::Receipt::read(&archive)?;
    let entry = read_dataforge(&archive)?;
    query(&archive, &entry, verified.record_index(), |_, facts| {
        verified.verify(&wire_row(facts)?)
    })?;
    watch
        .check()
        .map_err(|_| "Archive changed during cold cargo SDK verification")?;
    print_line("Cargo SDK: cold selective query reproduced one verified Ship base-facts row");
    Ok(())
}

fn read_dataforge(archive: &Archive) -> TestResult<DataForgeEntry> {
    DataForgeEntry::read(archive).map_err(|_| "read the original Archive DataForge Entry".into())
}

fn selected_record_index(forge: &DataForge<'_>) -> TestResult<u32> {
    let selection = std::env::var("P4KX_CARGO_SDK_RECORD")
        .map_err(|_| "configure the private physical Ship Record index or literal class")?;
    let Some(class) = selection.strip_prefix("class:") else {
        return selection
            .parse::<u32>()
            .map_err(|_| "invalid private physical Ship Record index".into());
    };
    if class.is_empty() || class.contains(['*', '?']) {
        return Err("the private Ship class must be literal".into());
    }
    let qualified = format!("{ENTITY_TYPE}.{class}");
    let mut records = forge.records().filter(|record| {
        record
            .name()
            .is_ok_and(|name| name.eq_ignore_ascii_case(&qualified))
    });
    let selected = records.next().ok_or("the private Ship class is absent")?;
    if records.next().is_some() {
        return Err("the private Ship class has multiple physical Records".into());
    }
    Ok(selected.index())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    index: u32,
) -> TestResult<(Filter, BTreeSet<u32>, Record<'source>)> {
    let record = forge
        .record(index)
        .ok_or("the physical Ship Record is absent")?;
    let path = record
        .record_path()
        .map_err(|_| "the selected Record path is unreadable")?;
    if record
        .type_name()
        .map_err(|_| "the selected Record type is unreadable")?
        != ENTITY_TYPE
        || path.is_empty()
        || path.contains(['*', '?'])
    {
        return Err("the selected Record lacks a usable literal Entity path".into());
    }
    let selected = forge
        .records_at_path(path)
        .map(|candidate| candidate.index())
        .collect::<BTreeSet<_>>();
    let wildcard = path
        .find(|character: char| character.is_ascii_alphabetic())
        .ok_or("the original path lacks an ASCII letter for an anchored candidate filter")?;
    let mut pattern = path.to_owned();
    pattern.replace_range(wildcard..=wildcard, "?");
    let filter = Filter::new(&pattern);
    let matched = forge
        .records()
        .filter(|candidate| {
            candidate
                .record_path()
                .is_ok_and(|path| filter.matches(path))
        })
        .map(|candidate| candidate.index())
        .collect::<BTreeSet<_>>();
    if !selected.contains(&index) || matched != selected {
        return Err("the scoped producer Filter extends beyond the exact original path".into());
    }
    Ok((filter, selected, record))
}

fn produce(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "generate the scoped normal cargo producer")?;
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read scoped producer JSON")?;
    verify_build(archive, &catalog)?;
    if catalog.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return Err("the scoped producer did not publish the current Schema".into());
    }
    Ok(catalog)
}

fn query<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    index: u32,
    inspect: impl FnOnce(&ShipSourceSnapshot<'_>, &ShipBaseFacts) -> TestResult<T>,
) -> TestResult<T> {
    with_ship_sources_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(index),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let [ship] = snapshot.rows() else {
                return Err("the scoped query must return one Ship".into());
            };
            let facts = ship.base_facts();
            let coverage = snapshot.coverage();
            if facts.ship_record_index().value() != Some(&index)
                || snapshot.build() != archive.build()
                || !matches!(snapshot.availability(), DatasetAvailability::Available)
                || !coverage.unreadable_headers.is_empty()
                || !coverage.unreadable_bodies.is_empty()
            {
                return Err(
                    "the scoped query differs from the nominated original Ship or Build".into(),
                );
            }
            inspect(&snapshot, facts)
        },
    )
    .map_err(|_| "read the physical Ship cargo SDK scope")?
}

fn compare_original<'source>(
    archive: &Archive,
    forge: &DataForge<'source>,
    record: &Record<'source>,
    facts: &ShipBaseFacts,
) -> TestResult {
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let ship = resolver
        .resolve(record)
        .map_err(|_| "resolve the original cargo Loadout")?;
    let cargo = facts.cargo();
    let total = legacy_total(forge, &ship);
    if facts.ship_guid().value() != Some(&record.guid())
        || !same_scu(cargo.scu().value().copied(), total.scu)
        || cargo.grids_with_issue().value() != Some(&total.issues)
    {
        return Err(
            "the SDK identity, SCU or issue count differs from original Loadout operands".into(),
        );
    }
    compare_original_mounts(&ship, record.index(), cargo)
}

fn compare_original_mounts(ship: &EntityNode, root: u32, cargo: &ShipCargoSource) -> TestResult {
    let original = ship.loadout_with_port_positions();
    let reached = original.iter().filter_map(|entry| {
        let item = entry.entity().filter(|item| legacy_grid(item))?;
        Some((entry, item))
    });
    let mut matched = 0;
    for (entry, item) in reached {
        let mount = cargo
            .mounts()
            .get(matched)
            .ok_or("the SDK omits an original CargoGrid mount")?;
        let position = mount
            .position()
            .value()
            .ok_or("a cargo mount lacks its physical position")?;
        let ordinals = entry
            .port_ordinals()
            .ok_or("an original cargo port lacks its occurrence")?;
        let observed = position
            .port_ordinals()
            .iter()
            .copied()
            .map(usize::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        if position.loadout_record_index() != root
            || observed != ordinals
            || mount.attachment().target_record_index().value().copied() != item.record_index()
        {
            return Err(
                "an SDK mount differs from the original ordered physical CargoGrid occurrence"
                    .into(),
            );
        }
        matched += 1;
    }
    if matched == 0 || matched != cargo.mounts().len() {
        return Err(
            "the nominated scope must match every original cargo mount and contain one or more"
                .into(),
        );
    }
    Ok(())
}

fn compare_catalog(
    catalog: &Value,
    admitted: &BTreeSet<u32>,
    index: u32,
    row: &Value,
) -> TestResult {
    let table = catalog
        .pointer(DATASET)
        .ok_or("the scoped producer lacks Ship base facts")?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("canonical Ship base facts rows")?;
    if table.get("availability").and_then(Value::as_str) != Some("available") {
        return Err("the normal scoped producer has no ShipBaseFacts table".into());
    }
    let mut checked = BTreeSet::new();
    for observed in rows {
        let record = u32::try_from(
            observed
                .pointer("/shipRecordIndex/value")
                .and_then(Value::as_u64)
                .ok_or("canonical Ship base facts physical Record")?,
        )?;
        if !admitted.contains(&record) || !checked.insert(record) {
            return Err(
                "the normal producer escaped its physical path scope or duplicated a row".into(),
            );
        }
        if record == index && observed != row {
            return Err("the scoped SDK and the normal producer differ on the selected row".into());
        }
    }
    if !checked.contains(&index) {
        return Err("the normal scoped producer omitted the nominated physical Ship".into());
    }
    let loadouts = catalog
        .get("loadouts")
        .and_then(Value::as_array)
        .ok_or("canonical Loadouts")?;
    let mut selected = loadouts.iter().filter(|loadout| {
        loadout
            .pointer("/sourceRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(u64::from(index))
    });
    let loadout = selected
        .next()
        .ok_or("the canonical physical Ship Loadout")?;
    if selected.next().is_some() {
        return Err("the scoped producer duplicated the physical Ship Loadout".into());
    }
    let mounts = row
        .pointer("/cargo/mounts")
        .and_then(Value::as_array)
        .ok_or("canonical cargo mounts")?;
    for mount in mounts {
        compare_port(loadout, mount)?;
    }
    Ok(())
}

fn compare_port(loadout: &Value, mount: &Value) -> TestResult {
    let ordinals = mount
        .pointer("/position/value/portOrdinals")
        .and_then(Value::as_array)
        .ok_or("the cargo mount's physical port ordinals")?;
    if ordinals.is_empty() {
        return Err("a cargo mount must address a resolved port occurrence".into());
    }
    let mut child = loadout;
    for ordinal in ordinals {
        let ordinal = usize::try_from(ordinal.as_u64().ok_or("physical port occurrence")?)?;
        child = child
            .get("ports")
            .and_then(Value::as_array)
            .and_then(|ports| ports.get(ordinal))
            .ok_or("the canonical Loadout omitted the selected mount position")?;
    }
    let target = mount
        .pointer("/attachment/targetRecordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("the cargo mount's physical target Record")?;
    if child
        .pointer("/sourceConstraints/attachedItem/targetRecordIndex/value")
        .and_then(Value::as_u64)
        != Some(target)
        || child.get("dockedVehicle").and_then(Value::as_bool) != Some(false)
    {
        return Err(
            "the cargo mount and its canonical port disagree on the physical attachment".into(),
        );
    }
    Ok(())
}
