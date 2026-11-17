// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Archive, Guid, SourceValue};
use rayon::ThreadPoolBuilder;

use super::instance::ContainerInstanceKey;
use super::{PlacementScope, Placements};
use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, with_entry_capture};
use crate::contract::StarmapObject;
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::SourceDisposition;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::{self, HarvestSurvey, Located, Objects};

#[path = "starmap_harvest_placement_fixture.rs"]
mod fixture;

use fixture::{CLAIM, PLANET, PRESET, fixture};

#[test]
fn harvest_walk_keeps_location_parity_without_general_placement_outputs()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive = fixture(dir.path())?;
    let planet = PLANET.parse()?;
    let claim = CLAIM.parse()?;
    let presets = HashSet::from([PRESET.parse()?]);
    let (full, full_locations, full_uses) = with_entry_capture(&archive, |capture| {
        let full = Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        let located = locate(&archive, &full, planet, claim, &presets, capture)?;
        Ok((full, located, capture.source_uses().registered_use_count()?))
    })?;
    let read_harvest = || {
        with_entry_capture(&archive, |capture| {
            let survey = HarvestSurvey::new(&presets);
            let harvest = Placements::read_for_harvest(&archive, None, capture, &survey)?;
            let objects = objects(planet, claim, &harvest, capture);
            let (located, _) = survey.finish(&archive, &harvest.opened, &objects, None, capture)?;
            Ok((
                harvest,
                located,
                capture.source_uses().registered_use_count()?,
            ))
        })
    };
    let (harvest, harvest_locations, harvest_uses) = ThreadPoolBuilder::new()
        .num_threads(1)
        .build()?
        .install(read_harvest)?;
    let (parallel, parallel_locations, _) = ThreadPoolBuilder::new()
        .num_threads(2)
        .build()?
        .install(read_harvest)?;
    assert_deterministic(
        (&harvest, &harvest_locations),
        (&parallel, &parallel_locations),
    )?;

    assert_harvest_route_parity(&archive, &full, &harvest, claim);
    assert_retained_locations(&harvest_locations, &full_locations, planet)?;
    assert_minimal_outputs(&full, &harvest, full_uses, harvest_uses);
    assert_routes(&archive, &harvest, claim)?;
    Ok(())
}

fn assert_harvest_route_parity(
    archive: &Archive,
    full: &Placements,
    harvest: &Placements,
    claim: Guid,
) {
    assert_eq!(
        harvest.opened,
        full.opened
            .iter()
            .filter(|opened| {
                archive.entries().get(opened.entry).is_some_and(|entry| {
                    entry.path().ends_with("tile.socpak")
                        || entry.path().ends_with("unreached.socpak")
                        || entry.path().ends_with("unreadable-detached.socpak")
                })
            })
            .cloned()
            .collect::<Vec<_>>(),
        "only reached positives plus the same eligible unreached inventory"
    );
    assert_eq!(
        harvest.counts, full.counts,
        "unchanged route classification"
    );
    assert_eq!(
        harvest.get(claim).map(|placement| placement.position),
        full.get(claim).map(|placement| placement.position),
        "the same authored claim placement",
    );
}

fn assert_retained_locations(
    harvest_locations: &Located,
    full_locations: &Located,
    planet: Guid,
) -> Result<(), serde_json::Error> {
    assert_eq!(
        serde_json::to_value(&harvest_locations.locations)?,
        serde_json::to_value(&full_locations.locations)?,
        "same logical location values and ordering",
    );
    assert_eq!(
        harvest_locations.locations.len(),
        2,
        "one repeated tile and one unreached location"
    );
    assert!(
        harvest_locations
            .locations
            .iter()
            .any(|row| row.starmap_object_guid.is_none()),
        "unreached location keeps no parent"
    );
    assert!(
        harvest_locations
            .locations
            .iter()
            .any(|row| row.starmap_object_guid == Some(CatalogGuid(planet))),
        "the mining marker resolves through its actual parent"
    );
    assert_eq!(
        observed_reads(harvest_locations),
        observed_reads(full_locations),
        "same physical SOC reads"
    );
    assert_unreadable_membership(harvest_locations, full_locations);
    Ok(())
}

fn assert_minimal_outputs(
    full: &Placements,
    harvest: &Placements,
    full_uses: usize,
    harvest_uses: usize,
) {
    assert_eq!(
        companion_reads(full),
        (true, true),
        "the baseline actually reads both companions"
    );
    assert_eq!(
        companion_reads(harvest),
        (false, false),
        "the harvest survey never reads companions"
    );
    assert!(
        harvest_uses < full_uses,
        "general occurrence cells do not enter the source registry"
    );
    assert!(
        harvest.instances.is_empty() && harvest.locations.is_empty(),
        "no general rows are materialized"
    );
    assert!(
        harvest.spools.containers.is_none() && harvest.spools.locations.is_none(),
        "no general row spools are created"
    );
    assert_eq!(
        harvest.harvest_occurrences.len(),
        2,
        "only the two reached occurrences of the admitted provider retain routes",
    );
}

fn assert_unreadable_membership(harvest: &Located, full: &Located) {
    let reads: Vec<_> = observed_reads(harvest)
        .into_iter()
        .filter(|entry| entry.operation == EntryOperation::SocPreset)
        .collect();
    let parents: HashSet<_> = reads
        .iter()
        .filter_map(|entry| entry.entry_index.zip(entry.container_entry_index))
        .collect();
    assert_eq!(
        (reads.len(), parents.len()),
        (5, 5),
        "one read per physical SOC"
    );
    let failed = reads
        .iter()
        .filter(|entry| entry.disposition == SourceDisposition::ReadFailed);
    assert_eq!(failed.count(), 2, "one failed read per unreadable SOC");
    assert_eq!(
        harvest.unreadable, full.unreadable,
        "same unreadable coverage as the full walk",
    );
    assert_eq!(
        harvest.unreadable.len(),
        2,
        "one reached and one detached failure"
    );
    for name in ["unreadable-reached.socpak", "unreadable-detached.socpak"] {
        assert!(
            harvest.unreadable.iter().any(|path| path.contains(name)),
            "coverage names {name}",
        );
    }
}

fn assert_deterministic(
    (serial, serial_locations): (&Placements, &Located),
    (parallel, parallel_locations): (&Placements, &Located),
) -> Result<(), serde_json::Error> {
    let routes = |rows: &Placements| {
        rows.harvest_occurrences
            .iter()
            .map(|row| ContainerInstanceKey::from_route(row.root_entry, row.route.as_deref()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        routes(serial),
        routes(parallel),
        "physical routes keep their order"
    );
    assert_eq!(
        serial.opened, parallel.opened,
        "candidate membership is stable"
    );
    assert_eq!(
        observed_reads(serial_locations),
        observed_reads(parallel_locations),
        "same physical observations",
    );
    assert_eq!(
        serial_locations.unreadable, parallel_locations.unreadable,
        "same coverage",
    );
    assert_eq!(
        serde_json::to_value(&serial_locations.locations)?,
        serde_json::to_value(&parallel_locations.locations)?,
        "logical rows keep their order",
    );
    Ok(())
}

fn observed_reads(read: &Located) -> Vec<&ArchiveEntryObservation> {
    let mut entries: Vec<_> = read.ledger.entries.iter().collect();
    entries.sort_unstable_by_key(|entry| {
        (
            entry.container_entry_index,
            entry.entry_index,
            entry.operation,
        )
    });
    entries
}

fn companion_reads(placements: &Placements) -> (bool, bool) {
    let paths = || {
        placements
            .ledger
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
    };
    (
        paths().any(|path| path.contains("hcd/") || path.contains("hcd\\")),
        paths().any(|path| path.ends_with(".entxml")),
    )
}

fn assert_routes(
    archive: &Archive,
    harvest: &Placements,
    claim: Guid,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tile_routes = harvest.harvest_occurrences.iter().filter(|occurrence| {
        archive
            .entries()
            .get(occurrence.opened.entry)
            .is_some_and(|entry| entry.path().ends_with("tile.socpak"))
    });
    let first = tile_routes.next().ok_or("first authored occurrence")?;
    let second = tile_routes.next().ok_or("second authored occurrence")?;
    assert!(
        tile_routes.next().is_none(),
        "two physical occurrences of the same Entry"
    );
    assert_eq!(
        first.opened, second.opened,
        "ordinary logical lookup coordinates are equal"
    );
    assert_eq!(
        first.opened.under,
        Some(claim),
        "physical routes retain their original marker"
    );
    assert_eq!(
        first.root_entry, second.root_entry,
        "same physical system root"
    );
    let left = first.route.as_deref().ok_or("first detached route")?;
    let right = second.route.as_deref().ok_or("second detached route")?;
    assert!(
        left.steps()
            .map(super::proof::PlacementStep::child_index)
            .ne(right.steps().map(super::proof::PlacementStep::child_index)),
        "different original Child routes remain distinguishable"
    );
    assert!(
        !harvest.harvest_occurrences.iter().any(|occurrence| {
            archive
                .entries()
                .get(occurrence.opened.entry)
                .is_some_and(|entry| entry.path().ends_with("unreached.socpak"))
        }),
        "an unreached physical entry never acquires a route"
    );
    Ok(())
}

fn locate(
    archive: &Archive,
    placements: &Placements,
    planet: Guid,
    claim: Guid,
    presets: &HashSet<Guid>,
    capture: &EntryCapture<'_>,
) -> Result<Located, CatalogError> {
    let objects = objects(planet, claim, placements, capture);
    harvest_locations::locate(
        archive,
        &placements.opened,
        presets,
        &objects,
        None,
        capture,
    )
}

fn objects(
    planet: Guid,
    claim: Guid,
    placements: &Placements,
    capture: &EntryCapture<'_>,
) -> Objects {
    Objects::new(&[
        object(planet, false, placements, capture),
        object(claim, true, placements, capture),
    ])
}

fn object(
    guid: Guid,
    mining_claim: bool,
    placements: &Placements,
    capture: &EntryCapture<'_>,
) -> StarmapObject<'static> {
    let placement = placements.get(guid);
    StarmapObject {
        guid: CatalogGuid(guid),
        record_index: u32::from(mining_claim),
        class_name: "synthetic",
        name: capture.source_uses().value(SourceValue::Absent, &[], true),
        r#type: None,
        system: None,
        entity_name: None,
        placed_under_guid: placement.and_then(|row| row.placed_under).map(CatalogGuid),
        position: placement.map(|row| row.position),
        placement_proof: None,
        mining_claim,
    }
}
