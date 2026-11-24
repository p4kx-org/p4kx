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
use super::{BoundMeasurement, Counts, SOURCES, TestResult, USES, measurement};

#[path = "real_archive_catalog_digest_index.rs"]
mod digest_index;
#[path = "real_archive_catalog_digest_runs.rs"]
mod digest_runs;
#[path = "real_archive_catalog_stream_wire.rs"]
mod wire;

use digest_index::DigestIndex;
use digest_runs::{DigestRuns, OriginDigest};
use wire::OrderedRows;
pub(super) use wire::{RowDigest, StreamOrigins};

pub(crate) struct StreamedComponentDigests<'directory> {
    rows: OrderedRows,
    uses: DigestRuns<'directory>,
    sources: DigestRuns<'directory>,
}

impl<'directory> StreamedComponentDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        Self {
            rows: OrderedRows::default(),
            uses: DigestRuns::new(directory),
            sources: DigestRuns::new(directory),
        }
    }

    pub(crate) fn retain_owner<Row: Serialize>(
        &mut self,
        rows: &[Row],
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
        inventory: &SourceInventory,
    ) -> TestResult {
        if rows.is_empty() {
            return Ok(());
        }
        let mut references = HashSet::new();
        for row in rows {
            let digest = row_fingerprint(row, &mut references)?;
            self.rows.append(slot(row)?, digest)?;
        }
        capture_inventory(
            inventory,
            references,
            |id, row| spill(&mut self.uses, &id, row),
            |id, row| spill(&mut self.sources, &id, row),
        )
    }
}

fn spill(runs: &mut DigestRuns<'_>, id: &str, row: Value) -> TestResult {
    runs.push(OriginDigest {
        id: OriginDigest::id_hash(id),
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
        self.watch.check()?;
        let rows = evidence.rows.finish();
        match availability {
            DatasetAvailability::Available => {}
            DatasetAvailability::Absent if rows.count == 0 => {}
            _ => return Err("the streamed component family has incomplete availability".into()),
        }
        let uses = DigestIndex::open(evidence.uses.finish()?)?;
        let sources = DigestIndex::open(evidence.sources.finish()?)?;
        let have_rows = rows.count != 0;
        if (uses.count() != 0) != have_rows || (sources.count() != 0) != have_rows {
            return Err("the streamed component scope has incomplete original evidence".into());
        }
        let mut capture = measurement::normal_capture();
        capture.keep(&format!("{path}/availability"));
        capture.project(&format!("{path}/rows"), Projection::StreamRows(rows));
        capture.project(USES, Projection::StreamOrigins(StreamOrigins(&uses)));
        capture.project(SOURCES, Projection::StreamOrigins(StreamOrigins(&sources)));
        let catalog = self.read_catalog(&capture)?;
        let availability = serde_json::to_value(availability)?;
        let table = catalog
            .pointer(path)
            .ok_or("missing Normal component table")?;
        if table.get("availability") != Some(&availability)
            || table.get("rows") != Some(&Value::from(rows.count))
            || catalog.pointer(USES) != Some(&Value::from(uses.count()))
            || catalog.pointer(SOURCES) != Some(&Value::from(sources.count()))
        {
            return Err("streamed Catalog coverage, counts or availability differ".into());
        }
        self.watch.check()?;
        Ok(Counts {
            rows: [rows.count],
            source_uses: usize::try_from(uses.count())?,
            physical_sources: usize::try_from(sources.count())?,
        })
    }
}
