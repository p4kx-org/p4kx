// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::flares::{FlareLibrarySelection, read_flare_libraries};
use p4kx_export::catalog_queries::localization::with_language_sources;
use p4kx_export::catalog_queries::particle_libraries::{
    ParticleLibrarySelection, read_particle_libraries,
};
use p4kx_export::catalog_queries::radar_audio::{RadarAudioSelection, read_radar_audio};
use p4kx_export::catalog_queries::shops::{ShopInventorySelection, read_shop_inventories};
use p4kx_export::catalog_queries::subsumption_events::{
    SubsumptionEventSelection, read_subsumption_events,
};
use p4kx_extraction::Archive;

use super::super::{
    FLARE_LIBRARIES, LANGUAGE_SOURCES, PARTICLE_LIBRARIES, RADAR_AUDIO, SHOP_INVENTORIES,
    SUBSUMPTION_EVENTS,
};
use super::{Route, TestResult, fixtures};

pub(super) fn read_radar_configuration(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_radar_audio(archive, RadarAudioSelection::All, None)?;
    Route::queried_rows(
        RADAR_AUDIO,
        &snapshot.rows,
        "physical configuration query available; installed producer parity open",
    )
}

pub(super) fn read_language_sources(archive: &Archive) -> TestResult<Route> {
    with_language_sources(
        archive,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            if snapshot
                .sources
                .get("de")
                .and_then(|source| source.value())
                .is_none()
            {
                return Err("the language metadata query must return a present source".into());
            }
            Route::queried_value(
                LANGUAGE_SOURCES,
                &snapshot.sources,
                "selective language Entry metadata available; installed producer parity open",
            )
        },
    )?
}

pub(super) fn read_flare_library(archive: &Archive) -> TestResult<Route> {
    let selection = FlareLibrarySelection::ArchivePath(fixtures::FLARE_LIBRARY);
    let snapshot = read_flare_libraries(archive, selection, None)?;
    Route::queried_rows(
        FLARE_LIBRARIES,
        &snapshot.rows,
        "synthetic canonical parity and installed source witness available; installed producer parity open",
    )
}

pub(super) fn read_particle_library(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_particle_libraries(archive, ParticleLibrarySelection::All, None)?;
    Route::queried_rows(
        PARTICLE_LIBRARIES,
        &snapshot.rows,
        "synthetic producer parity available; installed producer parity and memory evidence open",
    )
}

pub(super) fn read_subsumption_event(archive: &Archive) -> TestResult<Route> {
    let snapshot = read_subsumption_events(
        archive,
        SubsumptionEventSelection::ArchivePath(fixtures::SUBSUMPTION_FIXTURE),
        None,
    )?;
    Route::queried_rows(
        SUBSUMPTION_EVENTS,
        &snapshot.rows,
        "synthetic producer parity available; installed Event-ID and producer parity open",
    )
}

pub(super) fn read_shop_inventory(archive: &Archive) -> TestResult<Route> {
    let selection = ShopInventorySelection::ArchivePath(fixtures::OFFLINE_SHOP);
    let snapshot = read_shop_inventories(archive, selection, None)?;
    Route::queried_rows(
        SHOP_INVENTORIES,
        &snapshot.rows,
        "physical Entry and path query available; installed producer parity open",
    )
}
