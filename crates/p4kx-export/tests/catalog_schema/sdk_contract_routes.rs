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

#[expect(
    dead_code,
    reason = "The parity witness uses only the generator scope of the shared spawn fixture."
)]
#[path = "../it/spawns_fixture/mod.rs"]
mod spawns_fixture;
use spawns_fixture::generator_query as spawns;

#[path = "sdk_mission_broker_context_route.rs"]
mod broker_context;
#[path = "../it/catalog_contract_query_probe_fixture.rs"]
mod fixture;
#[path = "sdk_mission_flow_route.rs"]
mod mission_flows;
#[path = "sdk_mission_property_route.rs"]
mod mission_properties;

const CONTRACTS: &str = "/projections/contracts";
const TITLES: &str = "/projections/missionTitles";
const PROPERTY_SETS: &str = "/missionPropertySets";
const GENERATORS: &str = "/definitions/contractGenerators";
const REMAINING: &str =
    "typed query and synthetic producer parity verified; complete installed producer parity open";

pub(super) const DATASETS: &[&str] = &[
    CONTRACTS,
    TITLES,
    PROPERTY_SETS,
    GENERATORS,
    mission_properties::DATASET,
    mission_flows::DATASET,
    mission_flows::TEMPLATE_DEFINITIONS,
    mission_flows::TEMPLATE_CONTEXTS,
    mission_flows::BROKER_DEFINITIONS,
    broker_context::DATASET,
    spawns::SHIPS,
    spawns::ENTITIES,
    spawns::LOCATIONS,
    spawns::NPCS,
];

pub(super) fn read() -> TestResult<[Route; 14]> {
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
    let [flows, templates, brokers, template_contexts] = mission_flows::read()?;
    let [ships, entities, locations, npcs] = read_spawn_rows()?;
    Ok([
        contracts,
        titles,
        sets,
        generators,
        mission_properties::read()?,
        flows,
        templates,
        brokers,
        template_contexts,
        broker_context::read()?,
        ships,
        entities,
        locations,
        npcs,
    ])
}

fn read_spawn_rows() -> TestResult<[Route; 4]> {
    const REMAINING: &str = "Generator-scope producer parity verified; independent Template and Broker enumeration, installed parity and memory acceptance open";
    spawns::with_catalog_parity(|_, snapshot| {
        Ok([
            Route::queried_rows(spawns::SHIPS, &snapshot.ship_spawns, REMAINING)?,
            Route::queried_rows(spawns::ENTITIES, &snapshot.entity_spawns, REMAINING)?,
            Route::queried_rows(spawns::LOCATIONS, &snapshot.location_queries, REMAINING)?,
            Route::queried_rows(spawns::NPCS, &snapshot.npc_spawns, REMAINING)?,
        ])
    })
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
