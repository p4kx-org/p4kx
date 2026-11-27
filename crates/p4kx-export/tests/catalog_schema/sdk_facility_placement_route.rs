// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    AlignmentDependencyState, FacilityConfigurationKind, FacilityInstance,
    LoadoutFacilityPlacementSnapshot,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation, SourceUseRole, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::super::FACILITY_INSTANCES;
use super::{Route, TestResult};

#[path = "../it/catalog_facility_instance_fixture.rs"]
mod fixture;

#[path = "sdk_installed_configuration_route.rs"]
mod installed;
#[path = "sdk_loadout_facility_visit.rs"]
mod visit;

const SHIP_RECORD: u32 = 0;
const SEAT_RECORD: u32 = 1;
const DECK_RECORD: u32 = 2;
const DOCKED_RECORD: u32 = 3;
const UNSELECTED_RECORD: u32 = 4;

pub(super) fn read() -> TestResult<Vec<Route>> {
    Ok(vec![read_facilities()?, installed::read()?])
}

fn read_facilities() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("facility-placements.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", fixture::database().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = normal_placements(&archive, &entry)?;
    let queries = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    let snapshot = queries.read_loadout_facility_placements(SHIP_RECORD, None)?;
    compare(
        &snapshot,
        &catalog,
        SHIP_RECORD,
        &[&[0], &[1], &[2], &[2, 0]],
    )?;
    assert_eq!(
        snapshot
            .configuration_dependencies
            .iter()
            .map(|dependency| (dependency.record_index, dependency.state))
            .collect::<Vec<_>>(),
        [SHIP_RECORD, SEAT_RECORD, DECK_RECORD]
            .map(|index| (index, AlignmentDependencyState::Complete)),
        "docked and unresolved branches never consult configuration owners",
    );
    assert!(
        snapshot
            .loadout_coverage
            .incomplete_fields
            .contains(&SHIP_RECORD),
        "the omitted dangling Reference remains reported in the original Loadout coverage",
    );
    let docked = queries.read_loadout_facility_placements(DOCKED_RECORD, None)?;
    compare(&docked, &catalog, DOCKED_RECORD, &[&[], &[0]])?;
    let unselected = queries.read_loadout_facility_placements(UNSELECTED_RECORD, None)?;
    assert_eq!(
        unselected.loadout_state,
        SourceValueState::Absent,
        "an inventory definition cannot serve as a Ship root"
    );
    assert!(
        unselected.rows.is_empty(),
        "an unadmitted root has no placements"
    );
    assert!(
        unselected.configuration_dependencies.is_empty(),
        "an unadmitted root skips configuration reads"
    );

    visit::compare(&archive, &entry, &catalog)?;
    check_encrypted_ancestor()?;
    Route::queried_rows(
        FACILITY_INSTANCES,
        &snapshot.rows,
        "Complete Loadout visitor and selected Interior queries have synthetic parity. Complete installed producer parity remains open.",
    )
}

fn normal_placements(archive: &Archive, entry: &DataForgeEntry) -> TestResult<Vec<Value>> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(archive, entry, options.with_full_provenance())?.json(),
    )?;
    catalog
        .pointer("/placements/facilityInstances/rows")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "canonical FacilityInstance rows".into())
}

fn compare(
    snapshot: &LoadoutFacilityPlacementSnapshot,
    normal: &[Value],
    ship_record: u32,
    ports: &[&[u32]],
) -> TestResult {
    assert_eq!(
        snapshot.loadout_state,
        SourceValueState::Present,
        "the physical Loadout root resolved"
    );
    let selected: Vec<_> = normal
        .iter()
        .filter(|row| {
            row.pointer("/source/kind").and_then(Value::as_str) == Some("loadout")
                && row
                    .pointer("/shipRecordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(ship_record))
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        serde_json::to_value(&selected)?,
        "the typed SDK rows retain the canonical physical join and field provenance",
    );
    assert_eq!(
        snapshot.rows.len(),
        ports.len(),
        "the same admitted installations are retained"
    );
    check_positions(snapshot, ship_record, ports)
}

fn check_positions(
    snapshot: &LoadoutFacilityPlacementSnapshot,
    ship_record: u32,
    ports: &[&[u32]],
) -> TestResult {
    for (row, &expected) in snapshot.rows.iter().zip(ports) {
        assert_eq!(
            row.ship_record_index().value(),
            Some(&ship_record),
            "the placement retains its physical Ship Record"
        );
        let placement = row
            .loadout_position()
            .ok_or("the selected Loadout position")?;
        let position = placement.value().ok_or("the original port coordinates")?;
        assert_eq!(
            position.loadout_record_index(),
            ship_record,
            "the path starts at the same physical Ship Record"
        );
        assert_eq!(
            position.port_ordinals(),
            expected,
            "the path retains the original nested array positions"
        );
        assert!(
            placement
                .origin()
                .source_uses_refs()
                .iter()
                .all(|required| {
                    snapshot
                        .source_inventory
                        .iter_uses()
                        .any(|source_use| source_use.id() == required)
                }),
            "the SDK source inventory resolves every placement use",
        );
        if expected.is_empty() {
            assert!(
                row.attached_item().is_none(),
                "the Ship root links no attached item"
            );
        }
    }
    Ok(())
}

fn check_encrypted_ancestor() -> TestResult {
    let (_directory, archive) = fixture::encrypted_xml_archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = normal_placements(&archive, &entry)?;
    let snapshot = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_loadout_facility_placements(SHIP_RECORD, None)?;
    compare(&snapshot, &catalog, SHIP_RECORD, &[&[0], &[0, 0], &[1, 0]])?;
    let [deck, seat, rack_seat] = snapshot.rows.as_slice() else {
        return Err("the deck, its seat and the seat below the empty rack".into());
    };
    check_empty_parent_selector(&snapshot, rack_seat)?;
    for (row, kind, record) in [
        (deck, FacilityConfigurationKind::Inventory, DECK_RECORD),
        (seat, FacilityConfigurationKind::Facility, SEAT_RECORD),
        (rack_seat, FacilityConfigurationKind::Facility, SEAT_RECORD),
    ] {
        let position = row
            .loadout_position()
            .ok_or("the selected Loadout position")?;
        assert_eq!(
            position.origin().contains_encrypted_source(),
            Some(true),
            "the path was selected through original encrypted XML",
        );
        let mut owners = row.configurations();
        assert_eq!(
            owners.len(),
            1,
            "one original component is linked per fixture placement"
        );
        let (actual_kind, owner) = owners.next().ok_or("one original configuration")?;
        assert_eq!(
            actual_kind, kind,
            "the installed component retains its configuration kind"
        );
        assert_eq!(
            owner.record_index().value(),
            Some(&record),
            "the component link retains its physical Record"
        );
        assert_eq!(
            owner.guid().origin().contains_encrypted_source(),
            Some(false),
            "the configuration owner comes from the clear DataForge",
        );
    }
    Ok(())
}

fn check_empty_parent_selector(
    snapshot: &LoadoutFacilityPlacementSnapshot,
    seat: &FacilityInstance,
) -> TestResult {
    let xml = roxmltree::Document::parse(fixture::LOADOUT_XML_SOURCE)?;
    let rack = xml
        .descendants()
        .find(|node| node.attribute("portName") == Some("rack"))
        .ok_or("the original empty XML rack")?;
    let attribute = rack
        .attributes()
        .position(|attribute| attribute.name() == "portName")
        .ok_or("the exact original port name attribute")?;
    let expected = [u64::from(rack.id().get()), u64::try_from(attribute)?];
    let position = seat.loadout_position().ok_or("the seat's resolved path")?;
    let used = position.origin().source_uses_refs();
    assert!(
        snapshot.source_inventory.iter_uses().any(|source| {
            used.contains(source.id())
                && source.role() == SourceUseRole::Selector
                && matches!(
                    source.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "xmlAttribute" && indices.as_slice() == expected
                        && field == "@portName"
                )
        }),
        "the child placement uses the empty parent port's exact XML selector"
    );
    Ok(())
}
