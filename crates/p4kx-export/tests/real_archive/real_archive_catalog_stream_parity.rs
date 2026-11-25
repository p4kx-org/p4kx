// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde_json::Value;

use super::compare_rows::Projection;
use super::digest_parity::{PhysicalSlot, capture_inventory, fingerprint, row_fingerprint};
use super::{BoundMeasurement, Counts, SOURCES, ScopedTextProof, TestResult, USES, measurement};
use crate::catalog_capture::CatalogCapture;

#[path = "real_archive_catalog_definition_digests.rs"]
mod definitions;
#[path = "real_archive_catalog_digest_runs.rs"]
mod digest_runs;
#[path = "real_archive_loot_harvest_digests.rs"]
mod loot_harvest;
#[path = "real_archive_mission_configuration_digests.rs"]
mod mission;
#[path = "real_archive_container_placement_digests.rs"]
mod placement;
#[path = "real_archive_catalog_stream_wire.rs"]
mod wire;

pub(crate) use loot_harvest::{LootHarvestDataset, LootHarvestDigests};
pub(super) use mission::MissionDispositionRows;
pub(crate) use mission::{MissionDisposition, StreamedMissionDigests};
pub(crate) use placement::StreamedPlacementDigests;

pub(crate) use definitions::StreamedDefinitionDigests;
use digest_runs::{DigestRuns, OriginDigest};
pub(crate) use wire::DefinitionOwner;
use wire::OrderedRows;
pub(super) use wire::{RowDigest, StreamOrigins};

pub(crate) struct StreamedComponentDigests<'directory, const TABLES: usize = 1> {
    rows: [OrderedRows; TABLES],
    uses: DigestRuns<'directory>,
    sources: DigestRuns<'directory>,
}

impl<'directory> StreamedComponentDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        Self::for_tables(directory)
    }

    pub(crate) fn retain_owner<Row: Serialize>(
        &mut self,
        rows: &[Row],
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
        inventory: &SourceInventory,
    ) -> TestResult {
        self.retain_table_owner(0, rows, slot, inventory)
    }
}

impl<'directory, const TABLES: usize> StreamedComponentDigests<'directory, TABLES> {
    pub(crate) fn for_tables(directory: &'directory Path) -> Self {
        Self {
            rows: std::array::from_fn(|_| OrderedRows::default()),
            uses: DigestRuns::new(directory),
            sources: DigestRuns::new(directory),
        }
    }

    pub(crate) fn retain_table_owner<Row: Serialize>(
        &mut self,
        table: usize,
        rows: &[Row],
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
        inventory: &SourceInventory,
    ) -> TestResult {
        let ordered = self
            .rows
            .get_mut(table)
            .ok_or("unknown component comparison table")?;
        if rows.is_empty() {
            return Ok(());
        }
        let mut references = HashSet::new();
        for row in rows {
            let digest = row_fingerprint(row, &mut references)?;
            ordered.append(slot(row)?, digest)?;
        }
        capture_inventory(
            inventory,
            references,
            |id, row| spill(&mut self.uses, id, row),
            |id, row| spill(&mut self.sources, id, row),
        )
    }
}

fn spill(runs: &mut DigestRuns<'_>, id: impl Into<Box<str>>, row: Value) -> TestResult {
    runs.push(OriginDigest {
        id: id.into(),
        payload: fingerprint(row)?,
    })?;
    Ok(())
}

impl BoundMeasurement {
    pub(crate) fn verify_streamed_component_table(
        &self,
        path: &str,
        availability: DatasetAvailability,
        evidence: StreamedComponentDigests<'_>,
    ) -> TestResult<Counts<1>> {
        self.verify_streamed_component_tables([path], [availability], evidence)
    }

    pub(crate) fn verify_streamed_component_tables<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        availability: [DatasetAvailability; TABLES],
        evidence: StreamedComponentDigests<'_, TABLES>,
    ) -> TestResult<Counts<TABLES>> {
        self.verify_streamed_tables(paths, availability, evidence, |_| {}, |_| Ok(()))
    }

    fn verify_streamed_tables<const TABLES: usize>(
        &self,
        paths: [&str; TABLES],
        availability: [DatasetAvailability; TABLES],
        evidence: StreamedComponentDigests<'_, TABLES>,
        extend_capture: impl for<'scope> FnOnce(&mut CatalogCapture<Projection<'scope>>),
        verify_extra: impl FnOnce(&Value) -> TestResult,
    ) -> TestResult<Counts<TABLES>> {
        self.verify_streamed_scope(
            StreamedRowPaths::Tables(paths),
            availability,
            evidence,
            None,
            extend_capture,
            verify_extra,
        )
    }

    fn verify_streamed_scope<const TABLES: usize>(
        &self,
        paths: StreamedRowPaths<'_, TABLES>,
        availability: [DatasetAvailability; TABLES],
        evidence: StreamedComponentDigests<'_, TABLES>,
        texts: Option<&ScopedTextProof>,
        extend_capture: impl for<'scope> FnOnce(&mut CatalogCapture<Projection<'scope>>),
        verify_extra: impl FnOnce(&Value) -> TestResult,
    ) -> TestResult<Counts<TABLES>> {
        self.watch.check()?;
        let rows = evidence.rows.map(OrderedRows::finish);
        let uses = evidence.uses.finish()?;
        let sources = evidence.sources.finish()?;
        let have_rows = rows.iter().any(|row| row.count != 0);
        if (uses.count() != 0) != have_rows || (sources.count() != 0) != have_rows {
            return Err("the streamed component scope has incomplete original evidence".into());
        }
        let mut capture = measurement::normal_capture();
        paths.project(&mut capture, &availability, &rows)?;
        capture.project(USES, Projection::StreamOrigins(StreamOrigins(&uses)));
        capture.project(SOURCES, Projection::StreamOrigins(StreamOrigins(&sources)));
        if let Some(texts) = texts {
            texts.project(&mut capture);
        }
        extend_capture(&mut capture);
        let catalog = self.read_catalog(&capture)?;
        verify_extra(&catalog)?;
        if let Some(texts) = texts {
            texts.verify(&catalog)?;
        }
        paths.verify(&catalog, &availability, &rows)?;
        if catalog.pointer(USES) != Some(&Value::from(uses.count()))
            || catalog.pointer(SOURCES) != Some(&Value::from(sources.count()))
        {
            return Err("streamed Catalog source-use or physical-source counts differ".into());
        }
        self.watch.check()?;
        Ok(Counts {
            rows: rows.map(|row| row.count),
            source_uses: usize::try_from(uses.count())?,
            physical_sources: usize::try_from(sources.count())?,
        })
    }
}

#[derive(Clone, Copy)]
enum StreamedRowPaths<'scope, const TABLES: usize> {
    Tables([&'scope str; TABLES]),
    Arrays([&'scope str; TABLES]),
}

impl<const TABLES: usize> StreamedRowPaths<'_, TABLES> {
    fn project(
        &self,
        capture: &mut CatalogCapture<Projection<'_>>,
        availability: &[DatasetAvailability; TABLES],
        rows: &[RowDigest; TABLES],
    ) -> TestResult {
        match self {
            Self::Tables(paths) => project_tables(capture, paths, availability, rows),
            Self::Arrays(paths) => {
                for ((path, row), state) in paths.iter().zip(rows).zip(availability) {
                    require_availability(path, *state, row.count)?;
                    capture.project(path, Projection::StreamRows(*row));
                }
                Ok(())
            }
        }
    }

    fn verify(
        &self,
        catalog: &Value,
        availability: &[DatasetAvailability; TABLES],
        rows: &[RowDigest; TABLES],
    ) -> TestResult {
        match self {
            Self::Tables(paths) => verify_tables(catalog, paths, availability, rows),
            Self::Arrays(paths) => {
                for (path, row) in paths.iter().zip(rows) {
                    if catalog.pointer(path) != Some(&Value::from(row.count)) {
                        return Err(
                            format!("{path}: streamed Catalog array row count differs").into()
                        );
                    }
                }
                Ok(())
            }
        }
    }
}

fn project_tables<const TABLES: usize>(
    capture: &mut CatalogCapture<Projection<'_>>,
    paths: &[&str; TABLES],
    availability: &[DatasetAvailability; TABLES],
    rows: &[RowDigest; TABLES],
) -> TestResult {
    for ((path, row), state) in paths.iter().zip(rows).zip(availability) {
        require_availability(path, *state, row.count)?;
        capture.keep(&format!("{path}/availability"));
        capture.project(&format!("{path}/rows"), Projection::StreamRows(*row));
    }
    Ok(())
}

fn require_availability(path: &str, availability: DatasetAvailability, rows: usize) -> TestResult {
    match availability {
        DatasetAvailability::Available => Ok(()),
        DatasetAvailability::Absent if rows == 0 => Ok(()),
        _ => {
            Err(format!("{path}: the streamed component family has incomplete availability").into())
        }
    }
}

fn verify_tables<const TABLES: usize>(
    catalog: &Value,
    paths: &[&str; TABLES],
    availability: &[DatasetAvailability; TABLES],
    rows: &[RowDigest; TABLES],
) -> TestResult {
    for ((path, availability), rows) in paths.iter().zip(availability).zip(rows) {
        let table = catalog
            .pointer(path)
            .ok_or_else(|| format!("missing Normal component table {path}"))?;
        if table.get("availability") != Some(&serde_json::to_value(availability)?)
            || table.get("rows") != Some(&Value::from(rows.count))
        {
            return Err(
                format!("{path}: streamed Catalog row count or availability differs").into(),
            );
        }
    }
    Ok(())
}

#[test]
fn streamed_component_tables_keep_physical_slots_scoped_to_each_path() -> TestResult {
    let first = test_component_row("port");
    let second = test_component_row("bound");
    let rows = [test_row_digest(&first)?, test_row_digest(&second)?];
    let paths = ["/configurations/ports", "/configurations/bounds"];
    let available = [DatasetAvailability::Available; 2];
    let mut capture = measurement::normal_capture();
    project_tables(&mut capture, &paths, &available, &rows)?;
    let document = |port: &Value| {
        serde_json::json!({"configurations": {
            "ports": {"availability": "available", "rows": [port]},
            "bounds": {"availability": "available", "rows": [&second]}
        }})
    };
    let json = serde_json::to_vec(&document(&first))?;
    let observed = capture.read(json.as_slice())?;
    verify_tables(&observed, &paths, &available, &rows)?;
    let swapped = serde_json::to_vec(&document(&second))?;
    assert!(
        capture.read(swapped.as_slice()).is_err(),
        "the same physical slot in another family cannot replace this table's row"
    );
    assert!(
        verify_tables(
            &observed,
            &paths,
            &[DatasetAvailability::Absent, DatasetAvailability::Available],
            &rows,
        )
        .is_err(),
        "the retained declaration state is checked for each path"
    );
    Ok(())
}

#[cfg(test)]
fn test_component_row(label: &str) -> Value {
    serde_json::json!({
        "ownerRecordIndex": {"state": "present", "value": 7},
        "componentOrdinal": {"state": "present", "value": 2},
        "label": label
    })
}

#[cfg(test)]
fn test_row_digest(row: &Value) -> TestResult<RowDigest> {
    let slot = PhysicalSlot {
        record_index: 7,
        component_ordinal: 2,
    };
    let mut ordered = OrderedRows::default();
    ordered.append(slot, fingerprint(row.clone())?)?;
    Ok(ordered.finish())
}

#[test]
fn streamed_loadout_array_checks_canonical_physical_owners() -> TestResult {
    use serde_json::json;

    let owner = DefinitionOwner::Loadout;
    let entity_guid = json!({"state":"present","value":"00000000-0000-0000-0000-000000000007"});
    let first = json!({
        "entityGuid": &entity_guid,
        "sourceRecordIndex":{"state":"present","value":3}
    });
    let second = json!({
        "entityGuid": &entity_guid,
        "sourceRecordIndex":{"state":"present","value":8}
    });
    let mut expected = OrderedRows::with_definition(owner);
    for row in [&first, &second] {
        expected.append_encounter_row(owner.key(row)?, fingerprint(row.clone())?)?;
    }
    let rows = [expected.finish()];
    let paths = StreamedRowPaths::Arrays(["/loadouts"]);
    let available = [DatasetAvailability::Available];
    let mut capture = measurement::normal_capture();
    paths.project(&mut capture, &available, &rows)?;

    let complete = serde_json::to_vec(&json!({"loadouts":[first, second]}))?;
    let decoded = capture.read(complete.as_slice())?;
    paths.verify(&decoded, &available, &rows)?;
    let missing = capture.read(b"{}".as_slice())?;
    assert!(
        paths.verify(&missing, &available, &rows).is_err(),
        "a missing Loadout array differs from a matching array"
    );
    for changed in [
        json!({"loadouts":{"rows":[first, second]}}),
        json!({"loadouts":[second, first]}),
        json!({"loadouts":[first, first]}),
        json!({"loadouts":[first, {"recordIndex":8}]}),
        json!({"loadouts":[first, {"sourceRecordIndex":8}]}),
        json!({"loadouts":[first, {"sourceRecordIndex":{"state":"absent","value":8}}]}),
    ] {
        let bytes = serde_json::to_vec(&changed)?;
        assert!(
            capture.read(bytes.as_slice()).is_err(),
            "the physical Loadout array is exact: {changed}"
        );
    }
    Ok(())
}
