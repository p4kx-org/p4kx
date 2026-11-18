// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::io::BufReader;
use std::time::UNIX_EPOCH;

use p4kx_assembly::{ArchiveFiles, Attachment, EntityNode, LoadoutResolver};
use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, CatalogSummary, Compression, SCHEMA_VERSION,
    write_catalog_from,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, Record, RecordTree,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog_v3_consumer::present;
use crate::real_archive_cargo_grid_capacity::{
    exact_record, legacy_grid, legacy_total, same_scu, strict_operands,
};
use crate::real_archive_catalog_pipe::{self as pipe, CountingReader};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "mining_storage_source.rs"]
mod original;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SHIP: &str = "P4KX_CATALOG_MINING_SHIP";
const POD: &str = "P4KX_CATALOG_MINING_POD";
const FILTER: &str = "P4KX_CATALOG_MINING_FILTER";
const CONTROL: &str = "P4KX_CATALOG_MINING_CONTROL_SHIP";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and nominate the physical mining ship and pod Records"]
fn installed_mining_storage_stays_outside_ship_cargo_scu() -> TestResult {
    let inputs = Inputs::from_environment()?;
    let path = real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge")?;
    let result = compare(&archive, &entry, &inputs)?;
    watch
        .check()
        .map_err(|_| "Archive changed while verifying mining storage")?;
    print_line(&format!(
        "mining storage: physical resource containers={}, repeated pod ports={}, ordinary CargoGrid control={}, scoped writes={}",
        result.containers, result.occurrences, result.ordinary_control, result.scope_writes
    ));
    Ok(())
}

struct ResultCounts {
    containers: usize,
    occurrences: usize,
    ordinary_control: bool,
    scope_writes: usize,
}

fn compare(archive: &Archive, entry: &DataForgeEntry, inputs: &Inputs) -> TestResult<ResultCounts> {
    let forge = entry.open().map_err(|_| "open the original DataForge")?;
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let (ships_filtered, pods_filtered) = inputs.filters();
    let ship = exact_record(&forge, &ships_filtered, &inputs.ship)?;
    let pod_record = exact_record(&forge, &pods_filtered, &inputs.pod)?;
    let pod = pod_record
        .tree()
        .map_err(|_| "read the physical pod Record")?;
    let node = resolver
        .resolve(&ship)
        .map_err(|_| "read the original mining ship Loadout")?;
    let occurrences = original_ports(&node, pod_record)?;
    let mut scope_writes = 1;
    let (summary, mut catalog) = scoped_catalog(archive, entry, &ships_filtered)?;
    compare_ship(&forge, &catalog, ship, &node)?;
    compare_ports(ship, pod_record, &occurrences, &catalog)?;
    let containers = if inputs.filter.is_some() {
        catalog.configurations.compare_pod(entry, &pod, &summary)?
    } else {
        scope_writes += 1;
        let (summary, pods) = scoped_catalog(archive, entry, &pods_filtered)?;
        pods.configurations.compare_pod(entry, &pod, &summary)?
    };
    let ordinary_control = if let Some(control) = &inputs.control {
        let filter = Filter::new(control);
        let record = exact_record(&forge, &filter, control)?;
        let control_node = resolver
            .resolve(&record)
            .map_err(|_| "read the original ordinary ship Loadout")?;
        check_positive_legacy_grid(&forge, &control_node)?;
        if !ships_filtered.matches(control) {
            scope_writes += 1;
            catalog = scoped_catalog(archive, entry, &filter)?.1;
        }
        compare_ship(&forge, &catalog, record, &control_node)?;
        true
    } else {
        false
    };
    Ok(ResultCounts {
        containers,
        occurrences: occurrences.len(),
        ordinary_control,
        scope_writes,
    })
}

struct Inputs {
    ship: String,
    pod: String,
    filter: Option<Filter>,
    control: Option<String>,
}

impl Inputs {
    fn from_environment() -> TestResult<Self> {
        let env = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
        let ship = env(SHIP).ok_or("configure the exact mining ship Record path")?;
        let pod = env(POD).ok_or("configure the exact mining pod Record path")?;
        let filter = env(FILTER).map(|value| Filter::parse(&value)).transpose()?;
        if filter
            .as_ref()
            .is_some_and(|filter| !filter.matches(&ship) || !filter.matches(&pod))
        {
            return Err("the shared witness filter must match the mining ship and pod".into());
        }
        Ok(Self {
            ship,
            pod,
            filter,
            control: env(CONTROL),
        })
    }

    fn filters(&self) -> (Filter, Filter) {
        self.filter.as_ref().map_or_else(
            || (Filter::new(&self.ship), Filter::new(&self.pod)),
            |filter| (filter.clone(), filter.clone()),
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCatalog {
    schema_version: u32,
    source: Value,
    configurations: WireConfigurations,
    ships: Vec<Value>,
    loadouts: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireConfigurations {
    facilities: WireTable,
    resource_containers: WireTable,
}

impl WireConfigurations {
    fn compare_pod(
        &self,
        entry: &DataForgeEntry,
        pod: &RecordTree<'_>,
        summary: &CatalogSummary,
    ) -> TestResult<usize> {
        original::compare(
            entry,
            pod,
            &self.facilities.rows,
            &self.resource_containers.rows,
            summary,
        )
    }
}

#[derive(Deserialize)]
struct WireTable {
    rows: Vec<Value>,
}

fn scoped_catalog(
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
    )
    .map_err(|_| "write and decode the scoped mining storage catalog")?;
    if catalog.schema_version != SCHEMA_VERSION || bytes != summary.size() {
        return Err("the scoped result must reconcile catalog schema and byte count".into());
    }
    verify_build(archive, &json!({ "source": &catalog.source }))?;
    Ok((summary, catalog))
}

fn original_ports(ship: &EntityNode, pod: Record<'_>) -> TestResult<BTreeSet<String>> {
    let mut ports = BTreeSet::new();
    for (path, port) in ship.walk() {
        let Attachment::Entity(item) = port.attachment() else {
            continue;
        };
        if item.record_index() == Some(pod.index())
            && (legacy_grid(item) || item.guid() != pod.guid() || !ports.insert(path))
        {
            return Err(
                "the original mining pod must occupy a distinct non-cargo-grid port".into(),
            );
        }
    }
    if ports.len() < 2 {
        return Err(
            "the original Loadout must attach the same physical mining pod at several ports".into(),
        );
    }
    Ok(ports)
}

fn compare_ports(
    ship: Record<'_>,
    pod: Record<'_>,
    expected: &BTreeSet<String>,
    catalog: &WireCatalog,
) -> TestResult {
    let ship_guid = ship.guid().to_string();
    let mut loadouts = catalog.loadouts.iter().filter(|row| {
        text(row, "/entityGuid") == Some(ship_guid.as_str())
            && text(row, "/entityKind") == Some("ship")
    });
    let loadout = loadouts
        .next()
        .ok_or("expected the nominated mining ship Loadout")?;
    if loadouts.next().is_some() {
        return Err("the nominated mining ship Loadout must be unique".into());
    }
    let expected_guid = pod.guid().to_string();
    let mut actual = BTreeSet::new();
    let mut stack: Vec<_> = ports(loadout)?
        .iter()
        .map(|port| (String::new(), port))
        .collect();
    while let Some((mut path, port)) = stack.pop() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(text(port, "/name").ok_or("published Loadout port name")?);
        if port.get("dockedVehicle").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let attachment = port.pointer("/sourceConstraints/attachedItem");
        let target = attachment.and_then(|link| index(link, "/targetRecordIndex/value"));
        if target == Some(u64::from(pod.index())) {
            let link = attachment.ok_or("exact physical Loadout target")?;
            verify_attachment(link, &expected_guid)?;
            if !actual.insert(path.clone()) {
                return Err("the exact pod Loadout path occurs twice".into());
            }
        }
        stack.extend(ports(port)?.iter().map(|child| (path.clone(), child)));
    }
    if actual != *expected {
        return Err("published pod occurrences differ from the original physical Loadout".into());
    }
    Ok(())
}

fn ports(parent: &Value) -> TestResult<&[Value]> {
    parent
        .get("ports")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "published default Loadout children".into())
}

fn verify_attachment(link: &Value, expected_guid: &str) -> TestResult {
    let index_cell = link
        .get("targetRecordIndex")
        .ok_or("sourced target Record selector")?;
    let guid_cell = link.get("targetGuid").ok_or("sourced target GUID")?;
    if present(index_cell)?.as_u64().is_none()
        || present(guid_cell)?.as_str() != Some(expected_guid)
    {
        return Err("the Loadout link does not retain the original physical owner".into());
    }
    for cell in [index_cell, guid_cell] {
        if cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            return Err("the exact physical Loadout link needs observed source evidence".into());
        }
    }
    Ok(())
}

fn compare_ship(
    forge: &DataForge<'_>,
    catalog: &WireCatalog,
    ship: Record<'_>,
    node: &EntityNode,
) -> TestResult {
    let path = ship.record_path().map_err(|_| "read nominated ship path")?;
    let guid = ship.guid().to_string();
    let mut candidates = catalog
        .ships
        .iter()
        .filter(|row| index(row, "/recordIndex/value") == Some(u64::from(ship.index())));
    let row = candidates
        .next()
        .ok_or("expected the nominated physical Ship")?;
    let cargo = row.get("cargoScu").ok_or("legacy cargoScu field")?;
    let expected = legacy_total(forge, node);
    if candidates.next().is_some()
        || present(&row["recordIndex"])?.as_u64() != Some(u64::from(ship.index()))
        || present(&row["guid"])?.as_str() != Some(guid.as_str())
        || present(&row["recordPath"])?.as_str() != Some(path)
        || !(cargo.is_null() || cargo.is_number())
        || !same_scu(cargo.as_f64(), expected.scu)
        || index(row, "/cargoGridsWithIssue") != Some(expected.issues)
    {
        return Err("legacy ship cargo differs from the actual InventoryContainer operands".into());
    }
    Ok(())
}

fn check_positive_legacy_grid(forge: &DataForge<'_>, ship: &EntityNode) -> TestResult {
    for port in ship.loadout() {
        let Some(guid) = port
            .entity()
            .filter(|item| legacy_grid(item))
            .and_then(EntityNode::inventory_container)
        else {
            continue;
        };
        let mut matches = forge.records_by_guid(guid);
        let Some(container) = matches.next() else {
            continue;
        };
        if matches.next().is_none()
            && container.type_name().ok() == Some("InventoryContainer")
            && let Ok(tree) = container.tree()
            && let Ok(amount) = strict_operands(tree.root())
            && amount.scu() > 0.0
        {
            return Ok(());
        }
    }
    Err("the ordinary ship must have a physical CargoGrid with positive sourced capacity".into())
}

fn text<'value>(owner: &'value Value, path: &str) -> Option<&'value str> {
    owner.pointer(path)?.as_str()
}

fn index(owner: &Value, path: &str) -> Option<u64> {
    owner.pointer(path)?.as_u64()
}
