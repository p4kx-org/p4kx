// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::{
    LootDocumentAdmission, LootDocumentQuery, LootDocumentReachability, LootDocumentSelection,
    LootDocumentSnapshot, LootEntityContainerDocument,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability,
};
use p4kx_extraction::{Archive, SourceInventory};
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, Origins, TestResult, capture_rows, wire_row, with_bound_measurement,
};
use crate::real_archive_verification::print_line;

const TABLE: &str = "/declarations/lootEntityContainerDocuments";

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_loot_documents_match_the_same_complete_catalog_and_exact_member() -> TestResult {
    with_bound_measurement(verify)
}

fn verify(archive: &Archive, measurement: &BoundMeasurement) -> TestResult {
    let session = CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)
        .map_err(|_| "open the same-Build Loot document SDK session")?;
    let queries = session
        .parsed(None)
        .map_err(|_| "parse the original DataForge for Loot documents")?;
    let all = queries
        .read_loot_documents(LootDocumentQuery::new(LootDocumentSelection::All), None)
        .map_err(|_| "read the all-set Loot document SDK scope")?;
    observe(archive, &all, LootDocumentReachability::Enumeration)?;
    if !all.coverage.placements_complete {
        return Err("complete Loot document parity requires complete placement traversal".into());
    }
    let all_visited = all.coverage.visited_loot_containers;
    let sample = Sample::capture(&all)?;
    let mut sources = Origins::default();
    let table = capture_rows(
        all.availability,
        &all.rows,
        &all.source_inventory,
        &mut sources,
        physical_pair,
    )?;
    drop(all);

    let compared = measurement.verify_tables([TABLE], &[table], &sources)?;
    let [documents] = compared.rows;
    let exact = queries
        .read_loot_documents(LootDocumentQuery::new(sample.selection), None)
        .map_err(|_| "read an exact member from the verified Loot document set")?;
    observe(archive, &exact, LootDocumentReachability::Reached)?;
    sample.verify(&exact)?;
    print_line(&format!(
        "complete Loot document SDK parity: documents={documents} sourceUses={} physicalSources={} exactRows={} allVisited={} exactVisited={}",
        compared.source_uses,
        compared.physical_sources,
        exact.rows.len(),
        all_visited,
        exact.coverage.visited_loot_containers,
    ));
    Ok(())
}

fn observe(
    archive: &Archive,
    snapshot: &LootDocumentSnapshot,
    expected_reach: LootDocumentReachability,
) -> TestResult {
    let coverage = &snapshot.coverage;
    if archive.build().is_none()
        || snapshot.build.as_ref() != archive.build()
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || snapshot.rows.is_empty()
        || !coverage.directory_complete
        || coverage.admission != LootDocumentAdmission::Selected
        || coverage.reachability != expected_reach
        || !coverage.documents_complete
        || coverage.visited_loot_containers == 0
    {
        return Err("Loot document SDK scope is incomplete or uses a different Build".into());
    }
    Ok(())
}

struct Sample {
    selection: LootDocumentSelection,
    row: Value,
    origins: Origins,
}

impl Sample {
    fn capture(all: &LootDocumentSnapshot) -> TestResult<Self> {
        let row = all.rows.first().ok_or("original Loot document sample")?;
        let selection = LootDocumentSelection::Document {
            container_entry_index: row
                .containing_container()
                .value()
                .ok_or("sample lacks a physical containing Entry")?
                .entry_index(),
            document_entry_index: row
                .document()
                .value()
                .ok_or("sample lacks a physical nested Entry")?
                .entry_index(),
        };
        let (row, origins) = row_and_origins(row, &all.source_inventory)?;
        Ok(Self {
            selection,
            row,
            origins,
        })
    }

    fn verify(&self, exact: &LootDocumentSnapshot) -> TestResult {
        let [row] = exact.rows.as_slice() else {
            return Err(
                "the exact SDK request must return one original physical Loot document".into(),
            );
        };
        let (value, origins) = row_and_origins(row, &exact.source_inventory)?;
        if exact.selection != self.selection || value != self.row || origins != self.origins {
            return Err(
                "the exact SDK row or source closure differs from the proven All row".into(),
            );
        }
        Ok(())
    }
}

fn row_and_origins(
    row: &LootEntityContainerDocument,
    inventory: &SourceInventory,
) -> TestResult<(Value, Origins)> {
    let serialized = wire_row(row)?;
    let mut origins = Origins::default();
    origins.extend(inventory, std::slice::from_ref(&serialized))?;
    Ok((serialized, origins))
}

fn physical_pair(row: &Value) -> TestResult<(u64, u64)> {
    let outer = row
        .pointer("/containingContainer/value/entryIndex")
        .and_then(Value::as_u64)
        .ok_or("required physical Loot owner index")?;
    let inner = row
        .pointer("/document/value/entryIndex")
        .and_then(Value::as_u64)
        .ok_or("required physical nested document index")?;
    Ok((outer, inner))
}
