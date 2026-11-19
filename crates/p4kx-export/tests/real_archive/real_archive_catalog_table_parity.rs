// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::path::PathBuf;

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_extraction::{Archive, ArchiveWatch, SourceUseRole};
use serde::Serialize;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde_json::map::Entry;
use serde_json::{Map, Value};

use crate::catalog_capture::CatalogCapture;
use crate::real_archive_catalog_size::artifact;
use crate::real_archive_catalog_size::replay::bindings::Receipt;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const USES: &str = "/provenance/uses";
const SOURCES: &str = "/provenance/sources";

pub(crate) struct Counts {
    pub(crate) rows: [usize; 2],
    pub(crate) source_uses: usize,
    pub(crate) physical_sources: usize,
}

pub(crate) fn verify_two_tables(
    paths: [&str; 2],
    read_sdk: impl FnOnce(&Archive, &mut Origins) -> TestResult<[Table; 2]>,
) -> TestResult<Counts> {
    let directory = std::env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(PathBuf::from)
        .ok_or("configure the completed Catalog measurement")?;
    let binding = Receipt::read(&directory)?.binding()?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the matching Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the matching Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    binding.check_archive(&archive)?;

    let mut origins = Origins::default();
    let tables = read_sdk(&archive, &mut origins)?;
    watch.check()?;

    let mut capture = CatalogCapture::default();
    for path in [
        "/schemaVersion",
        "/source",
        "/completeness",
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
    ]
    .into_iter()
    .chain(paths)
    {
        capture.keep(path);
    }
    capture.project(USES, SelectedOrigins(&origins.uses));
    capture.project(SOURCES, SelectedOrigins(&origins.sources));
    let catalog = artifact::read_completed(&directory, |input| Ok(capture.read(input)?))?;
    binding.check_catalog(&catalog)?;

    for (path, table) in paths.into_iter().zip(&tables) {
        compare_table(&catalog, path, table)?;
    }
    origins.verify(&catalog)?;
    watch.check()?;
    Ok(Counts {
        rows: tables.each_ref().map(|table| table.rows.len()),
        source_uses: origins.uses.len(),
        physical_sources: origins.sources.len(),
    })
}

pub(crate) struct Table {
    availability: Value,
    rows: Vec<Value>,
}

pub(crate) fn capture_table<Row: Serialize, Id: Eq + Hash>(
    sdk: &EquipmentSnapshot<'_, Row>,
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
    let rows: Vec<Value> = serde_json::from_slice(&serde_json::to_vec(sdk.rows())?)?;
    let mut physical = HashSet::new();
    for row in &rows {
        if !physical.insert(identity(row)?) {
            return Err("the SDK reports a duplicate physical row identity".into());
        }
    }
    origins.extend(sdk, &rows)?;
    Ok(Table {
        availability: serde_json::to_value(sdk.availability())?,
        rows,
    })
}

fn compare_table(catalog: &Value, path: &str, sdk: &Table) -> TestResult {
    let table = catalog.pointer(path).ok_or("required full Catalog table")?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("required full Catalog rows")?;
    if table.get("availability") != Some(&sdk.availability) || rows.len() != sdk.rows.len() {
        return Err(format!("Catalog and SDK inventories differ at {path}").into());
    }
    for (index, (actual, expected)) in rows.iter().zip(&sdk.rows).enumerate() {
        if let Some(field) = first_difference(actual, expected) {
            return Err(format!("Catalog and SDK differ at {path}/rows/{index}{field}").into());
        }
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

#[derive(Default)]
pub(crate) struct Origins {
    uses: Map<String, Value>,
    sources: Map<String, Value>,
}

impl Origins {
    fn extend<Row>(&mut self, sdk: &EquipmentSnapshot<'_, Row>, rows: &[Value]) -> TestResult {
        let mut needed = HashSet::new();
        for row in rows {
            collect_references(row, &mut needed);
        }
        if needed.is_empty() {
            return Err("expected original cell provenance".into());
        }
        let inventory = sdk.source_inventory();
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
        for (path, expected) in [(USES, &self.uses), (SOURCES, &self.sources)] {
            if catalog.pointer(path).and_then(Value::as_object) != Some(expected) {
                return Err(
                    format!("Catalog and SDK contributing provenance differ at {path}").into(),
                );
            }
        }
        Ok(())
    }
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

#[derive(Clone, Copy)]
struct SelectedOrigins<'scope>(&'scope Map<String, Value>);

impl<'de> DeserializeSeed<'de> for SelectedOrigins<'_> {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for SelectedOrigins<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut selected = Map::new();
        while let Some(row) = sequence.next_element::<Value>()? {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| de::Error::custom("Catalog provenance row has no identifier"))?;
            if self.0.contains_key(id) && selected.insert(id.to_owned(), row).is_some() {
                return Err(de::Error::custom(
                    "duplicate selected Catalog provenance identifier",
                ));
            }
        }
        Ok(Value::Object(selected))
    }
}

#[test]
fn selected_provenance_keeps_rows_and_rejects_duplicate_keys() -> TestResult {
    let expected = Map::from_iter([(
        "selected".to_owned(),
        serde_json::json!({"id":"selected","role":"payload"}),
    )]);
    let mut capture = CatalogCapture::default();
    capture.project(USES, SelectedOrigins(&expected));
    let input = br#"{"provenance":{"uses":[{"id":"unused"},{"id":"selected","role":"payload"}]}}"#;
    let selected = capture.read(input.as_slice())?;
    assert_eq!(
        selected.pointer(USES).and_then(Value::as_object),
        Some(&expected),
        "only the exact selected source-use row is retained",
    );
    let duplicate = br#"{"provenance":{"uses":[{"id":"selected"},{"id":"selected"}]}}"#;
    assert!(
        capture.read(duplicate.as_slice()).is_err(),
        "duplicate selected source rows cannot prove provenance closure",
    );
    Ok(())
}
