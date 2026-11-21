// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::BufReader;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, CatalogSummary, Compression, SCHEMA_VERSION,
    write_catalog_from,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, InputScope, SourceEncryption,
    SourceInput, SourceScope, SourceUses, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog_v3_consumer;
use crate::real_archive_catalog_pipe::{self as pipe, CountingReader};
use crate::real_archive_item_durability::Trace;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "cargo_grid_capacity_census.rs"]
mod census;
#[path = "cargo_grid_capacity_original.rs"]
mod original;
#[path = "cargo_grid_capacity_ships.rs"]
mod ships;

use original::Grid;
pub(super) use original::{exact_record, strict_operands};
pub(super) use ships::{legacy_grid, legacy_total};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FILTER: &str = "P4KX_CATALOG_CARGO_GRID_FILTER";
const MINING: &str = "P4KX_CATALOG_CARGO_MINING_ITEM";
const CONTROL: &str = "P4KX_CATALOG_CARGO_CONTROL_ITEM";
const SHIPS: &str = "P4KX_CATALOG_CARGO_SHIPS";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K for the raw CargoGrid flag census"]
fn installed_grid_flag_source_census() -> TestResult {
    census::run()
}

#[test]
#[ignore = "real Archive: configure Data.p4k and the exact cargo Grid witness Record paths"]
fn installed_grid_capacity_matches_original_records_and_loadout_ports() -> TestResult {
    let filter = Filter::parse(&required_env(FILTER)?)?;
    let ship_paths = std::env::var(SHIPS).unwrap_or_default();
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let grids = [
        Grid::read(&forge, &filter, &required_env(MINING)?, true)?,
        Grid::read(&forge, &filter, &required_env(CONTROL)?, false)?,
    ];
    let witness_ships = ships::select(&archive, &forge, &filter, &ship_paths, &grids)?;
    let selection = forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
        .count();
    print_line(&format!(
        "cargo capacity witness selected Record paths={selection}"
    ));
    let (summary, catalog) = catalog(&archive, &entry, &filter)?;
    let channel = catalog
        .source
        .get("channel")
        .and_then(Value::as_str)
        .filter(|channel| matches!(*channel, "LIVE" | "PTU"))
        .ok_or("set the Archive to the installed LIVE or PTU Build")?;
    verify_build(&archive, &json!({"source": &catalog.source}))?;
    let invisible = compare_grids(
        &entry,
        &grids,
        &catalog.configurations.facilities.rows,
        &summary,
    )?;
    let counts = ships::compare(&catalog, &witness_ships, &grids)?;
    watch
        .check()
        .map_err(|_| "Archive changed during the cargo grid comparison")?;
    print_line(&format!(
        "{channel}: 2 exact cargo grids (mining=1 control=1 invisible={invisible}), placements={}, repeated grid slots={}, legacy ships={}",
        counts.placements, counts.repeats, counts.ships,
    ));
    Ok(())
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name)
        .ok()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("configure {name}").into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCatalog {
    schema_version: u32,
    source: Value,
    configurations: WireConfigurations,
    projections: WireProjections,
    placements: WirePlacements,
    ships: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireProjections {
    ship_base_facts: WireTable,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireConfigurations {
    facilities: WireTable,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePlacements {
    facility_instances: WireTable,
}

#[derive(Deserialize)]
struct WireTable {
    rows: Vec<Value>,
}

fn catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: &Filter,
) -> TestResult<(CatalogSummary, WireCatalog)> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (summary, (catalog, bytes)) = pipe::transfer(
        |reader| {
            let mut reader = BufReader::new(CountingReader::new(reader));
            let catalog: WireCatalog = serde_json::from_reader(&mut reader)?;
            Ok::<_, serde_json::Error>((catalog, reader.into_inner().byte_count()))
        },
        |sink| write_catalog_from(archive, entry, &options, Compression::None, || Ok(sink)),
    )?;
    if catalog.schema_version != SCHEMA_VERSION || bytes != summary.size() {
        return Err(
            "the streamed cargo result must reconcile its catalog schema and byte accounting"
                .into(),
        );
    }
    Ok((summary, catalog))
}

fn compare_grids(
    entry: &DataForgeEntry,
    originals: &[Grid<'_>; 2],
    rows: &[Value],
    summary: &CatalogSummary,
) -> TestResult<usize> {
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("expected the physical Archive DataForge Entry".into());
    };
    let source_uses = SourceUses::new();
    let scope = source_uses.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let marker = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let [left, right] = originals;
    let rows = [grid_row(rows, left)?, grid_row(rows, right)?];
    let selected = selected_uses(summary, &rows)?;
    let uses = selected
        .iter()
        .map(|row| {
            Ok((
                row.get("id")
                    .and_then(Value::as_str)
                    .ok_or("source-use ID")?,
                row,
            ))
        })
        .collect::<TestResult<HashMap<_, _>>>()?;
    let mut invisible = 0;
    for (grid, row) in originals.iter().zip(rows) {
        compare_grid(grid, row, &scope, &uses, marker)?;
        invisible += usize::from(matches!(
            grid.flag("invisible")?,
            Some(Original::Bool(true))
        ));
    }
    Ok(invisible)
}

fn grid_row<'row>(rows: &'row [Value], original: &Grid<'_>) -> TestResult<&'row Value> {
    let mut matched = rows.iter().filter(|row| {
        text(row, "/kind") == Some("cargoGrid")
            && index(row, "/ownerRecordIndex/value")
                == Some(u64::from(original.item.record_index()))
            && index(row, "/componentOrdinal/value")
                == u64::try_from(original.facility_ordinal).ok()
    });
    let row = matched
        .next()
        .ok_or("the exact physical cargoGrid configuration is absent")?;
    if matched.next().is_some()
        || text(row, "/ownerGuid/value") != Some(original.item.guid().to_string().as_str())
    {
        return Err("the cargoGrid configuration must match the unique physical Item".into());
    }
    Ok(row)
}

fn compare_grid(
    original: &Grid<'_>,
    row: &Value,
    scope: &SourceScope,
    uses: &HashMap<&str, &Value>,
    marker: Option<bool>,
) -> TestResult {
    let item = Trace::of(&original.item, uses, scope, marker);
    let container = Trace::of(&original.container, uses, scope, marker);
    for name in ["miningOnly", "invisible"] {
        compare_flag(
            &item,
            original.flag(name)?,
            row.get(name).ok_or("cargoGrid flag carrier")?,
        )?;
    }
    let capacity = row.get("capacity").ok_or("cargo grid capacity")?;
    if text(capacity, "/status") != Some("available")
        || index(capacity, "/storageComponentOrdinal/value")
            != u64::try_from(original.storage_ordinal).ok()
        || index(capacity, "/containerRecordIndex/value")
            != Some(u64::from(original.container.record_index()))
    {
        return Err("capacity does not select the physical storage sibling and Container".into());
    }
    let Some(scu) = capacity.get("scu") else {
        return Err("immediate SCU carrier".into());
    };
    let expected = strict_operands(original.container.root())?;
    if !same_scu(
        catalog_v3_consumer::present(scu)?.as_f64(),
        Some(expected.scu()),
    ) || scu.get("containsEncryptedSource") != Some(&json!(marker))
    {
        return Err(
            "the SCU value or its immediate marker differs from the physical source".into(),
        );
    }
    item.part(scu, original.components()?, "$value", "selector")?;
    item.part(scu, original.reference()?, "$value", "selector")?;
    container.header(scu, "recordIndex", "selector")?;
    expected.compare_uses(&container, scu)
}

fn compare_flag<'source>(
    trace: &Trace<'_, 'source>,
    original: Option<&Original<'source>>,
    carrier: &Value,
) -> TestResult {
    match original {
        Some(raw @ Original::Bool(flag))
            if catalog_v3_consumer::present(carrier)?.as_bool() == Some(*flag) =>
        {
            trace.check(carrier, raw, "payload")
        }
        None if text(carrier, "/state") == Some("absent") => Ok(()),
        Some(Original::Null) if text(carrier, "/state") == Some("null") => Ok(()),
        _ => Err("the cargoGrid flag differs from its original declaration".into()),
    }
}

pub(super) fn selected_uses(summary: &CatalogSummary, rows: &[&Value]) -> TestResult<Vec<Value>> {
    let mut wanted = HashSet::new();
    for row in rows {
        collect_references(row, &mut wanted)?;
    }
    summary
        .source_inventory()
        .iter_uses()
        .filter(|row| wanted.contains(row.id().as_str()))
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

fn collect_references<'wire>(value: &'wire Value, wanted: &mut HashSet<&'wire str>) -> TestResult {
    match value {
        Value::Object(fields) => {
            if let Some(references) = fields.get("sourceUsesRefs") {
                for id in references.as_array().ok_or("source-use reference list")? {
                    wanted.insert(id.as_str().ok_or("source-use reference ID")?);
                }
            }
            for child in fields.values() {
                collect_references(child, wanted)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_references(child, wanted)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn same_scu(actual: Option<f64>, expected: Option<f64>) -> bool {
    match (actual, expected) {
        (Some(actual), Some(expected)) => {
            (actual - expected).abs() <= 4.0 * f64::EPSILON * expected.abs().max(1.0)
        }
        (None, None) => true,
        _ => false,
    }
}

fn index(value: &Value, path: &str) -> Option<u64> {
    value.pointer(path).and_then(Value::as_u64)
}

fn text<'value>(value: &'value Value, path: &str) -> Option<&'value str> {
    value.pointer(path).and_then(Value::as_str)
}
