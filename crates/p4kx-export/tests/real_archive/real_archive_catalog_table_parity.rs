// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::hash::Hash;
use std::io::Read;
use std::path::PathBuf;

use p4kx_channels::PublicPatches;
use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_extraction::{Archive, ArchiveWatch, SourceInventory};
use serde::Serialize;
use serde::de::DeserializeSeed;
use serde_json::map::Entry;
use serde_json::{Map, Value};

use crate::catalog_capture::CatalogCapture;
use crate::real_archive_catalog_preflight;
use crate::real_archive_catalog_size::artifact;
use crate::real_archive_catalog_size::replay::bindings::Binding;

#[path = "real_archive_catalog_compare_rows.rs"]
mod compare_rows;

#[path = "real_archive_catalog_digest_parity.rs"]
mod digest_parity;
#[path = "real_archive_json_digest.rs"]
mod json_digest;
#[path = "real_archive_loadout_census.rs"]
mod loadout_census;
#[path = "real_archive_catalog_measurement.rs"]
mod measurement;
#[path = "real_archive_catalog_stream_parity.rs"]
mod stream_parity;

pub(crate) use digest_parity::{DigestedInventory, DigestedTable, PhysicalSlot, fingerprint};
pub(crate) use json_digest::update_json_digest;
pub(crate) use loadout_census::ProducerSeal;
pub(crate) use measurement::{with_bound_measurement, with_bound_measurement_from_producer};
pub(crate) use stream_parity::{
    DefinitionOwner, LootHarvestDataset, LootHarvestDigests, MissionDisposition,
    StreamedComponentDigests, StreamedDefinitionDigests, StreamedInteriorDigests,
    StreamedMissionDigests, StreamedPlacementDigests,
};

#[path = "real_archive_catalog_selected_origins.rs"]
mod selected_origins;

#[path = "real_archive_catalog_selected_array.rs"]
mod selected_array;
use selected_array::check_row_array_count;

#[path = "real_archive_catalog_selected_row.rs"]
mod selected_row;

#[path = "real_archive_catalog_selected_texts.rs"]
mod selected_texts;
pub(crate) use selected_texts::ScopedTextProof;

#[path = "real_archive_catalog_selected_ships.rs"]
mod selected_ships;
pub(crate) use selected_row::check_selected_row;

pub(crate) use compare_rows::first_difference;
use compare_rows::{ComparedRows, Projection, RowSelection};
use selected_origins::SelectedOrigins;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const USES: &str = "/provenance/uses";
const SOURCES: &str = "/provenance/sources";

pub(crate) struct Counts<const TABLES: usize> {
    pub(crate) rows: [usize; TABLES],
    pub(crate) source_uses: usize,
    pub(crate) physical_sources: usize,
}

pub(crate) fn verify_tables<const TABLES: usize>(
    paths: [&str; TABLES],
    read_sdk: impl FnOnce(&Archive, &mut Origins) -> TestResult<[Table; TABLES]>,
) -> TestResult<Counts<TABLES>> {
    with_bound_measurement(|archive, measurement| {
        let mut origins = Origins::default();
        let tables = read_sdk(archive, &mut origins)?;
        measurement.verify_tables(paths, &tables, &origins)
    })
}

pub(crate) struct BoundMeasurement {
    directory: PathBuf,
    binding: Binding,
    watch: ArchiveWatch,
}

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K, P4KX_CATALOG_MEASUREMENT_DIR and P4KX_LAUNCHER_LOGS"]
fn installed_public_patch_matches_the_same_catalog_build() -> TestResult {
    let logs = std::env::var_os("P4KX_LAUNCHER_LOGS")
        .map(PathBuf::from)
        .ok_or("configure the launcher log directory")?;
    if !["log.log", "log.old.log"]
        .iter()
        .any(|name| logs.join(name).is_file())
    {
        return Err("the snapshot contains no launcher log".into());
    }
    with_bound_measurement(|archive, measurement| {
        let build = archive.build().ok_or("missing installed Build identity")?;
        let patches = PublicPatches::standard_report();
        if !patches.is_complete() {
            return Err("the launcher log scan was incomplete".into());
        }
        let source = real_archive_catalog_preflight::source(&measurement.directory)?;
        measurement.binding.check_source_value(&source)?;
        let expected = serde_json::to_value(patches.value().of(build))?;
        if source.pointer("/build/publicPatch") != Some(&expected) {
            return Err("the Catalog public patch differs from the exact launcher lookup".into());
        }
        Ok(())
    })
}

impl BoundMeasurement {
    pub(crate) fn verify_row_array(&self, path: &str, expected: &[Value]) -> TestResult {
        self.watch.check()?;
        let mut capture = measurement::normal_capture();
        capture.project(
            path,
            Projection::Rows(ComparedRows::bare_array(path, expected, RowSelection::All)),
        );
        let catalog = artifact::read_completed(&self.directory, |input| Ok(capture.read(input)?))?;
        self.binding.check_catalog(&catalog)?;
        check_row_array_count(&catalog, path, expected.len())?;
        self.watch.check()?;
        Ok(())
    }

    pub(crate) fn verify_tables<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        tables: &[Table; TABLES],
        origins: &Origins,
    ) -> TestResult<Counts<TABLES>> {
        self.verify_tables_and_arrays(paths, tables, &[], origins)
    }

    pub(crate) fn verify_tables_and_arrays<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        tables: &[Table; TABLES],
        arrays: &[(&str, &[Value])],
        origins: &Origins,
    ) -> TestResult<Counts<TABLES>> {
        self.verify_table_scope(paths, tables, arrays, origins, None)
    }

    pub(crate) fn verify_table_scope<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        tables: &[Table; TABLES],
        arrays: &[(&str, &[Value])],
        origins: &Origins,
        texts: Option<&ScopedTextProof>,
    ) -> TestResult<Counts<TABLES>> {
        self.watch.check()?;
        let mut capture = measurement::normal_capture();
        if let Some(texts) = texts {
            texts.project(&mut capture);
        }
        for (path, table) in paths.iter().zip(tables) {
            capture.keep(&format!("{path}/availability"));
            capture.project(
                &format!("{path}/rows"),
                Projection::Rows(ComparedRows::table(path, &table.rows, table.selection)),
            );
        }
        for &(path, rows) in arrays {
            capture.project(
                path,
                Projection::Rows(ComparedRows::bare_array(path, rows, RowSelection::All)),
            );
        }
        capture.project(USES, Projection::Origins(SelectedOrigins(&origins.uses)));
        capture.project(
            SOURCES,
            Projection::Origins(SelectedOrigins(&origins.sources)),
        );
        let catalog = artifact::read_completed(&self.directory, |input| Ok(capture.read(input)?))?;
        self.binding.check_catalog(&catalog)?;
        for (path, table) in paths.into_iter().zip(tables) {
            check_table(&catalog, path, table)?;
        }
        for &(path, rows) in arrays {
            check_row_array_count(&catalog, path, rows.len())?;
        }
        origins.verify(&catalog)?;
        if let Some(texts) = texts {
            texts.verify(&catalog)?;
        }
        self.watch.check()?;
        Ok(Counts {
            rows: tables.each_ref().map(|table| table.rows.len()),
            source_uses: origins.uses.len(),
            physical_sources: origins.sources.len(),
        })
    }
}

pub(crate) struct Table {
    availability: Value,
    rows: Vec<Value>,
    selection: RowSelection,
}

impl Table {
    pub(crate) fn physical_ship(
        availability: DatasetAvailability,
        ship: u32,
        rows: Vec<Value>,
    ) -> TestResult<Self> {
        Self::from_wire(availability, rows, RowSelection::PhysicalShip(ship))
    }

    pub(crate) fn rows(&self) -> &[Value] {
        &self.rows
    }

    fn from_wire(
        availability: DatasetAvailability,
        rows: Vec<Value>,
        selection: RowSelection,
    ) -> TestResult<Self> {
        Ok(Self {
            availability: serde_json::to_value(availability)?,
            rows,
            selection,
        })
    }
}

pub(crate) fn capture_table<Row: Serialize, Id: Eq + Hash, Dependencies>(
    sdk: &EquipmentSnapshot<'_, Row, Dependencies>,
    archive: &Archive,
    origins: &mut Origins,
    identity: impl Fn(&Value) -> TestResult<Id>,
) -> TestResult<Table> {
    let coverage = sdk.coverage();
    if sdk.build() != archive.build()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || sdk.rows().is_empty()
    {
        return Err("the SDK must read the matching Build and every complete original row".into());
    }
    capture_rows(
        sdk.availability(),
        sdk.rows(),
        sdk.source_inventory(),
        origins,
        identity,
    )
}

pub(crate) fn capture_rows<Row: Serialize, Id: Eq + Hash>(
    availability: DatasetAvailability,
    sdk_rows: &[Row],
    inventory: &SourceInventory,
    origins: &mut Origins,
    identity: impl Fn(&Value) -> TestResult<Id>,
) -> TestResult<Table> {
    let mut rows = Vec::with_capacity(sdk_rows.len());
    let mut physical = HashSet::new();
    for row in sdk_rows {
        let row = wire_row(row)?;
        if !physical.insert(identity(&row)?) {
            return Err("the SDK reports a duplicate physical row identity".into());
        }
        rows.push(row);
    }
    if !rows.is_empty() {
        origins.extend(inventory, &rows)?;
    }
    Table::from_wire(availability, rows, RowSelection::All)
}

pub(crate) fn wire_row(row: &impl Serialize) -> TestResult<Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(row)?)?)
}

fn check_table(catalog: &Value, path: &str, sdk: &Table) -> TestResult {
    let table = catalog.pointer(path).ok_or("required full Catalog table")?;
    let count = Value::from(sdk.rows.len());
    if table.get("availability") != Some(&sdk.availability) || table.get("rows") != Some(&count) {
        return Err(format!("Catalog and SDK inventories differ at {path}").into());
    }
    Ok(())
}

pub(crate) fn collect_references<'a>(value: &'a Value, references: &mut HashSet<&'a str>) {
    match value {
        Value::Object(fields) => {
            if let Some(ids) = fields.get("sourceUsesRefs").and_then(Value::as_array) {
                references.extend(ids.iter().filter_map(Value::as_str));
            }
            for field in fields.values() {
                collect_references(field, references);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_references(value, references);
            }
        }
        _ => {}
    }
}

#[derive(Default, PartialEq)]
pub(crate) struct Origins {
    uses: Map<String, Value>,
    sources: Map<String, Value>,
}

impl Origins {
    pub(crate) fn contributing_counts(&self) -> (usize, usize) {
        (self.uses.len(), self.sources.len())
    }

    pub(crate) fn extend(&mut self, inventory: &SourceInventory, rows: &[Value]) -> TestResult {
        let mut needed = HashSet::new();
        for row in rows {
            collect_references(row, &mut needed);
        }
        digest_parity::capture_inventory(
            inventory,
            needed,
            |id, row| retain(&mut self.uses, id, row),
            |id, row| retain(&mut self.sources, id, row),
        )
    }

    /// Captures selected Catalog sections and their expected contributing rows.
    pub(crate) fn read_scope(&self, input: impl Read, paths: &[&str]) -> serde_json::Result<Value> {
        let mut capture = CatalogCapture::default();
        for section in paths {
            capture.keep(section);
        }
        for (path, expected) in self.inventories() {
            capture.project(path, SelectedOrigins(expected));
        }
        capture.read(input)
    }

    pub(crate) fn verify(&self, catalog: &Value) -> TestResult {
        for (path, expected) in self.inventories() {
            verify_inventory(path, catalog.pointer(path), expected)?;
        }
        Ok(())
    }

    pub(crate) fn verify_in_document(&self, catalog: &Value) -> TestResult {
        for (path, expected) in self.inventories() {
            let entries = catalog
                .pointer(path)
                .ok_or("missing Catalog provenance array")?;
            let actual = SelectedOrigins(expected).deserialize(entries)?;
            verify_inventory(path, Some(&actual), expected)?;
        }
        Ok(())
    }

    const fn inventories(&self) -> [(&'static str, &Map<String, Value>); 2] {
        [(USES, &self.uses), (SOURCES, &self.sources)]
    }
}

fn verify_inventory(
    path: &str,
    actual: Option<&Value>,
    expected: &Map<String, Value>,
) -> TestResult {
    if actual.and_then(Value::as_object) != Some(expected) {
        return Err(format!("Catalog and SDK contributing provenance differ at {path}").into());
    }
    Ok(())
}

fn retain(rows: &mut Map<String, Value>, id: String, row: Value) -> TestResult {
    match rows.entry(id) {
        Entry::Occupied(existing) if existing.get() == &row => Ok(()),
        Entry::Occupied(_) => Err("the SDK produced conflicting source observations".into()),
        Entry::Vacant(slot) => {
            slot.insert(row);
            Ok(())
        }
    }
}
