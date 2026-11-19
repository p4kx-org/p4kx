// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, ArchiveWatch, SourceUseRole};
use serde::Serialize;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde_json::map::Entry;
use serde_json::{Map, Value};

use super::super::collect_references;
use super::{TestResult, first_cargo_difference, record_index};
use crate::catalog_capture::CatalogCapture;
use crate::real_archive_catalog_size::artifact;
use crate::real_archive_catalog_size::replay::bindings::Receipt;
use crate::real_archive_verification::print_line;

const LOADING: &str = "/configurations/cargoLoadingGlobals";
const PRICES: &str = "/configurations/shopCommodityGlobals";
const USES: &str = "/provenance/uses";
const SOURCES: &str = "/provenance/sources";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_cargo_sdk_matches_the_same_complete_catalog() -> TestResult {
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
    let (loading, prices) = read_sdk(&archive, &mut origins)?;
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
        LOADING,
        PRICES,
    ] {
        capture.keep(path);
    }
    capture.project(USES, SelectedOrigins(&origins.uses));
    capture.project(SOURCES, SelectedOrigins(&origins.sources));
    let catalog = artifact::read_completed(&directory, |input| Ok(capture.read(input)?))?;
    binding.check_catalog(&catalog)?;

    compare_table(&catalog, LOADING, &loading)?;
    compare_table(&catalog, PRICES, &prices)?;
    origins.verify(&catalog)?;
    watch.check()?;
    print_line(&format!(
        "cargo SDK parity: loadingRows={} shopRows={} sourceUses={} physicalSources={}",
        loading.rows.len(),
        prices.rows.len(),
        origins.uses.len(),
        origins.sources.len(),
    ));
    Ok(())
}

struct Table {
    availability: Value,
    rows: Vec<Value>,
}

fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<(Table, Table)> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let loading =
        parsed.with_cargo_loading_globals(CatalogRecordSelection::All, None, |sdk| {
            capture_table(&sdk, archive, origins)
        })??;
    let prices =
        parsed.with_shop_commodity_globals(CatalogRecordSelection::All, None, |sdk| {
            capture_table(&sdk, archive, origins)
        })??;
    Ok((loading, prices))
}

fn capture_table<Row: Serialize>(
    sdk: &EquipmentSnapshot<'_, Row>,
    archive: &Archive,
    origins: &mut Origins,
) -> TestResult<Table> {
    let coverage = sdk.coverage();
    if sdk.build() != archive.build()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || sdk.rows().is_empty()
    {
        return Err(
            "the SDK must read the matching Build and every complete original Cargo row".into(),
        );
    }
    let rows: Vec<Value> = serde_json::from_slice(&serde_json::to_vec(sdk.rows())?)?;
    let mut physical = HashSet::new();
    for row in &rows {
        if !physical.insert(record_index(row)?) {
            return Err("the SDK reports a duplicate physical Cargo Record".into());
        }
    }
    origins.extend(sdk, &rows)?;
    Ok(Table {
        availability: serde_json::to_value(sdk.availability())?,
        rows,
    })
}

fn compare_table(catalog: &Value, path: &str, sdk: &Table) -> TestResult {
    let table = catalog
        .pointer(path)
        .ok_or("required full Catalog Cargo table")?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("required full Catalog Cargo rows")?;
    if table.get("availability") != Some(&sdk.availability) || rows.len() != sdk.rows.len() {
        return Err(format!("Catalog and SDK inventories differ at {path}").into());
    }
    for (index, (actual, expected)) in rows.iter().zip(&sdk.rows).enumerate() {
        if let Some(field) = first_cargo_difference(actual, expected) {
            return Err(format!("Catalog and SDK differ at {path}/rows/{index}{field}").into());
        }
    }
    Ok(())
}

#[derive(Default)]
struct Origins {
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
            return Err("expected original Cargo cell provenance".into());
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
                return Err("a Cargo value refers to a noncontributing observation".into());
            }
            retain(&mut self.uses, id.to_owned(), serde_json::to_value(used)?)?;
            pending.push(used.source().as_str().to_owned());
        }
        if !needed.is_empty() {
            return Err("a Cargo value lacks its referenced SDK source use".into());
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
        Entry::Occupied(_) => Err("the SDK produced conflicting Cargo observations".into()),
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
fn selected_cargo_provenance_keeps_rows_and_rejects_duplicate_keys() -> TestResult {
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
