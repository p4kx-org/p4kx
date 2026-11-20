// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::{Route, TestResult};

#[path = "../it/catalog_contract_query_probe_fixture.rs"]
mod fixture;

const CONTRACTS: &str = "/projections/contracts";
const TITLES: &str = "/projections/missionTitles";
const PROPERTY_SETS: &str = "/missionPropertySets";
const GENERATORS: &str = "/definitions/contractGenerators";
const REMAINING: &str =
    "typed query and synthetic producer parity verified; complete installed producer parity open";

pub(super) const DATASETS: &[&str] = &[CONTRACTS, TITLES, PROPERTY_SETS, GENERATORS];

pub(super) fn read() -> TestResult<[Route; 4]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("contracts.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::database()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let [contracts, titles, sets] = parsed.with_contracts(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult<[Route; 3]> {
            Ok([
                compare(&catalog, CONTRACTS, &snapshot.rows)?,
                compare(&catalog, TITLES, &snapshot.mission_titles)?,
                compare(&catalog, PROPERTY_SETS, &snapshot.mission_property_sets)?,
            ])
        },
    )??;
    let generators =
        parsed.with_contract_generators(CatalogRecordSelection::All, None, |snapshot| {
            compare(&catalog, GENERATORS, &snapshot.rows)
        })??;
    Ok([contracts, titles, sets, generators])
}

fn compare<Row: Serialize + JsonSchema>(
    catalog: &Value,
    dataset: &'static str,
    rows: &[Row],
) -> TestResult<Route> {
    let table = catalog
        .pointer(dataset)
        .ok_or("canonical Catalog dataset")?;
    let canonical = table.get("rows").unwrap_or(table);
    assert_eq!(
        &serde_json::to_value(rows)?,
        canonical,
        "the same synthetic owner produces the canonical rows at {dataset}"
    );
    Route::queried_rows(dataset, rows, REMAINING)
}
