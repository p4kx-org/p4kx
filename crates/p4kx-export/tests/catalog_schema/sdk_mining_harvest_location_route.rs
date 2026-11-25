// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_export::mining_locations::{
    HarvestableLocation, HarvestableOccurrence, HarvestableOccurrenceRoute, MiningLocationOptions,
    MiningLocationSnapshot,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{DataForgeEntry, Language};
use serde_json::Value;

use super::{Route, TestResult};

#[path = "../../src/mining_location_reader_fixture.rs"]
mod fixture;

const DATASET: &str = "/harvestableLocations";

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let archive = fixture::archive(&directory.path().join("Data.p4k"))?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog_from(
        &archive,
        &entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let snapshot = session
        .parsed(None)?
        .read_mining_locations(MiningLocationOptions::new(Language::English, None))?;
    let rows = serde_json::to_value(&snapshot.harvestable_locations)?;
    assert_eq!(
        &rows,
        catalog
            .pointer(DATASET)
            .ok_or("normal producer harvestable locations")?,
        "the SDK uses the canonical logical row and order"
    );
    assert_occurrences(&snapshot)?;
    Route::queried_rows(
        DATASET,
        &snapshot.harvestable_locations,
        "installed LIVE and PTU logical parity witnessed; integrated application memory open",
    )
}

fn assert_occurrences(snapshot: &MiningLocationSnapshot) -> TestResult {
    let [placed, unplaced] = snapshot.harvestable_locations.as_slice() else {
        return Err("the repeated tile and unreached container form two logical locations".into());
    };
    assert!(
        placed.starmap_object_guid.is_some(),
        "the reached tile retains its object"
    );
    assert!(
        unplaced.starmap_object_guid.is_none(),
        "the directory-only row has no rooted object"
    );
    let mut routes = Vec::new();
    let mut unreached = 0;
    for occurrence in &snapshot.harvestable_occurrences {
        let origin = occurrence.preset_guid.origin();
        assert!(
            origin.source_set_complete(),
            "the fixture supplies the original preset source"
        );
        let references = origin.source_uses_refs();
        assert!(
            !references.is_empty()
                && references.iter().all(|reference| snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|used| used.id() == reference)),
            "the physical occurrence retains original SOC evidence"
        );
        let logical = match &occurrence.route {
            HarvestableOccurrenceRoute::Reached(route) => {
                assert!(
                    route.root.value().is_some(),
                    "a reached occurrence retains its physical root"
                );
                routes.push(route.id.value().ok_or("physical rooted occurrence")?);
                placed
            }
            HarvestableOccurrenceRoute::Unreached => {
                unreached += 1;
                unplaced
            }
            _ => return Err("unexpected fixture route disposition".into()),
        };
        assert_occurrence_row(occurrence, logical);
    }
    let [first, second] = routes.as_slice() else {
        return Err("the same tile is reached through two selected Child routes".into());
    };
    assert_ne!(
        first, second,
        "the two physical occurrences have distinct routes"
    );
    assert_eq!(unreached, 1, "directory-only occurrence");
    Ok(())
}

fn assert_occurrence_row(occurrence: &HarvestableOccurrence, logical: &HarvestableLocation) {
    assert_eq!(
        occurrence.preset_guid.value(),
        Some(&logical.preset_guid),
        "the logical row retains this preset"
    );
    assert_eq!(
        occurrence.container_path.value(),
        Some(&logical.container),
        "the logical row retains this container path"
    );
    assert_eq!(
        occurrence.starmap_object_guid, logical.starmap_object_guid,
        "the physical occurrence joins the same starmap object"
    );
    assert!(
        occurrence.container.value().is_some(),
        "the original container Entry is bound"
    );
}
