// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;

use p4kx_channels::PublicPatches;
use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_extraction::{Archive, ArchiveWatch, SourceInventory, SourceUseRole};
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

#[path = "real_archive_catalog_measurement.rs"]
mod measurement;
pub(crate) use measurement::{with_bound_measurement, with_bound_measurement_from_producer};

#[path = "real_archive_catalog_selected_origins.rs"]
mod selected_origins;

use compare_rows::{ComparedRows, Projection};
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
        let mut capture = normal_capture();
        capture.project(
            path,
            Projection::Rows(ComparedRows::bare_array(path, expected)),
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
        self.watch.check()?;
        let mut capture = normal_capture();
        for (path, table) in paths.iter().zip(tables) {
            capture.keep(&format!("{path}/availability"));
            capture.project(
                &format!("{path}/rows"),
                Projection::Rows(ComparedRows::new(path, &table.rows)),
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
        origins.verify(&catalog)?;
        self.watch.check()?;
        Ok(Counts {
            rows: tables.each_ref().map(|table| table.rows.len()),
            source_uses: origins.uses.len(),
            physical_sources: origins.sources.len(),
        })
    }
}

fn normal_capture<'scope>() -> CatalogCapture<Projection<'scope>> {
    let mut capture = CatalogCapture::default();
    for path in [
        "/schemaVersion",
        "/source",
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
    ] {
        capture.keep(path);
    }
    capture.project("/completeness", Projection::CompleteSelection);
    capture
}

fn check_row_array_count(catalog: &Value, path: &str, expected: usize) -> TestResult {
    if catalog.pointer(path) != Some(&Value::from(expected)) {
        return Err(format!("missing or incomplete Catalog row array at {path}").into());
    }
    Ok(())
}

pub(crate) struct Table {
    availability: Value,
    rows: Vec<Value>,
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
    Ok(Table {
        availability: serde_json::to_value(availability)?,
        rows,
    })
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

pub(crate) fn first_difference(expected: &Value, actual: &Value) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, child) in expected {
                match actual.get(key) {
                    Some(actual) => {
                        if let Some(path) = first_difference(child, actual) {
                            return Some(format!("/{key}{path}"));
                        }
                    }
                    None => return Some(format!("/{key}")),
                }
            }
            Some("/additionalKey".into())
        }
        (Value::Array(expected), Value::Array(actual)) if expected.len() == actual.len() => {
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(at, (left, right))| {
                    first_difference(left, right).map(|path| format!("/{at}{path}"))
                })
        }
        _ => Some(String::new()),
    }
}

#[derive(Default, PartialEq)]
pub(crate) struct Origins {
    uses: Map<String, Value>,
    sources: Map<String, Value>,
}

impl Origins {
    pub(crate) fn extend(&mut self, inventory: &SourceInventory, rows: &[Value]) -> TestResult {
        let mut needed = HashSet::new();
        for row in rows {
            collect_references(row, &mut needed);
        }
        if needed.is_empty() {
            return Err("expected original cell provenance".into());
        }
        let mut pending = Vec::new();
        for used in inventory.iter_uses() {
            let id = used.id().as_str();
            if !needed.remove(id) {
                continue;
            }
            if !matches!(
                used.role(),
                SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
            ) {
                return Err("a value refers to a noncontributing observation".into());
            }
            retain(&mut self.uses, id.to_owned(), serde_json::to_value(used)?)?;
            pending.push(used.source().as_str().to_owned());
        }
        if !needed.is_empty() {
            return Err("a value lacks its referenced SDK source use".into());
        }
        let sources: HashMap<_, _> = inventory
            .iter_sources()
            .map(|source| (source.id().as_str(), source))
            .collect();
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if visited.contains(&id) {
                continue;
            }
            let source = sources
                .get(id.as_str())
                .ok_or("missing SDK physical source or ancestor")?;
            let row = serde_json::to_value(source)?;
            for ancestor in row
                .get("ancestors")
                .and_then(Value::as_array)
                .ok_or("missing SDK physical source ancestry")?
            {
                pending.push(
                    ancestor
                        .as_str()
                        .ok_or("invalid ancestor reference")?
                        .to_owned(),
                );
            }
            retain(&mut self.sources, id.clone(), row)?;
            visited.insert(id);
        }
        Ok(())
    }

    fn verify(&self, catalog: &Value) -> TestResult {
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
