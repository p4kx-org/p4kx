// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::resource_topology::{
    ItemRoomAssociationsSnapshot, NetworkEndpoint, ResourceNetworkLinksSnapshot,
    ResourceTopologyDependencyRole, RoomAssociationsSnapshot, read_resource_topology_placements,
    with_item_room_associations, with_resource_network_links,
};
use p4kx_export::catalog_queries::equipment::{AlignmentDependencyState, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, SourceWeakTarget,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use serde::Serialize;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_topology_declarations_fixture.rs"]
mod fixture;

#[test]
fn topology_sdk_declarations_match_canonical_rows_and_original_uses() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    parsed.with_resource_network_links(selection, None, |snapshot| {
        check_links(&snapshot)?;
        parity(&snapshot, &catalog, "/declarations/resourceNetworkLinks")
    })??;
    parsed.with_room_associations(selection, None, |snapshot| {
        check_vehicle_rooms(&snapshot)?;
        parity(&snapshot, &catalog, "/declarations/roomAssociations")
    })??;
    parsed.with_item_room_associations(selection, None, |snapshot| {
        check_item_rooms(&snapshot)?;
        parity(&snapshot, &catalog, "/declarations/itemRoomAssociations")
    })??;
    Ok(())
}

#[test]
fn topology_sdk_selects_only_the_requested_physical_owner() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    with_resource_network_links(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "item room owner contributes no link declaration"
            );
        },
    )?;
    Ok(())
}

#[test]
fn topology_sdk_reports_unreadable_owned_room_extensions() -> TestResult {
    let database = fixture::database_with_room_extension(fixture::bad_string());
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    with_item_room_associations(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| -> TestResult {
            check_item_rooms(&snapshot)?;
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [1],
                "an unreadable retained field belongs to the selected room declaration"
            );
            Ok(())
        },
    )??;
    Ok(())
}

#[test]
fn topology_placement_sdk_distinguishes_unknown_declarations_from_empty_scope() -> TestResult {
    let database =
        fixture::database_with_link_endpoint(fixture::bad_string(), fixture::bad_string());
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let snapshot = read_resource_topology_placements(&archive, 0, None)?;
    assert!(
        snapshot.declaration_dependencies.iter().any(|source| {
            source.record_index == 0
                && source.role == ResourceTopologyDependencyRole::LinkDeclarations
                && source.state == AlignmentDependencyState::IncompleteProjection
        }),
        "an unreadable selected link makes the declaring role incomplete"
    );
    assert!(
        snapshot
            .declaration_dependencies
            .iter()
            .all(|source| source.record_index == 0),
        "the unrelated malformed Item Record is outside the topology scope"
    );
    let bindings = snapshot.placements.resource_network_link_bindings();
    assert!(
        bindings.rows.iter().any(|row| {
            matches!(
                row.port2().value(),
                Some(NetworkEndpoint::Unavailable { .. })
            )
        }),
        "unreadable authored endpoint evidence cannot become a missing endpoint"
    );
    Ok(())
}

fn check_links(snapshot: &ResourceNetworkLinksSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one declared port container".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&0),
        "original ship Record"
    );
    let links = row
        .internal_hardpoint_links()
        .value()
        .ok_or("original hardpoint slots")?;
    let [link] = links.as_slice() else {
        return Err("one original hardpoint link".into());
    };
    let link = link.value().ok_or("original hardpoint link")?;
    assert_eq!(
        link.link_ordinal().value(),
        Some(&0),
        "uncompacted link slot"
    );
    assert_eq!(
        link.port1().value(),
        Some(&"mount"),
        "authored named endpoint"
    );
    assert_eq!(
        link.port1().origin().contains_encrypted_source(),
        Some(true),
        "endpoint origin"
    );
    let weak_links = row
        .internal_resource_links()
        .value()
        .ok_or("original weak links")?;
    let [weak] = weak_links.as_slice() else {
        return Err("one original weak link".into());
    };
    let weak = weak.value().ok_or("original weak link")?;
    let target = weak.port1().value().ok_or("original owned-port target")?;
    assert!(
        matches!(target.target(), SourceWeakTarget::Instance { .. })
            && target.instance_steps().is_some(),
        "the weak reference retains its original owned-port location"
    );
    Ok(())
}

fn check_vehicle_rooms(snapshot: &RoomAssociationsSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original vehicle component".into());
    };
    let rooms = row.object_containers().value().ok_or("original rooms")?;
    let [room] = rooms.as_slice() else {
        return Err("one original room".into());
    };
    let room = room.value().ok_or("original room")?;
    assert_eq!(
        room.container_ordinal().value(),
        Some(&0),
        "original room slot"
    );
    assert_eq!(
        room.file_name().value(),
        Some(&fixture::ROOM_FILE),
        "authored container locator"
    );
    assert_eq!(
        room.bone_name().origin().contains_encrypted_source(),
        Some(true),
        "bone origin"
    );
    Ok(())
}

fn check_item_rooms(snapshot: &ItemRoomAssociationsSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original item room".into());
    };
    assert_eq!(
        row.owner().record_index().value(),
        Some(&1),
        "original item Record"
    );
    assert_eq!(
        row.object_container().value(),
        Some(&fixture::ITEM_FILE),
        "authored item room locator"
    );
    Ok(())
}

fn parity<Row: Serialize>(
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
    dataset: &str,
) -> TestResult {
    assert!(
        snapshot.requested_languages().is_empty()
            && snapshot.coverage().incomplete_fields.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty(),
        "only the selected declaration fields affect coverage"
    );
    let table = proof::at(catalog, dataset)?;
    let source_inventory = json!({"provenance":snapshot.source_inventory()});
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "shared declaration availability"
    );
    let rows = table.get("rows").ok_or("original Catalog rows")?;
    assert_eq!(
        proof::with_origins(serde_json::to_value(snapshot.rows())?, &source_inventory)?,
        proof::with_origins(rows.clone(), catalog)?,
        "complete declaration rows and original source uses"
    );
    Ok(())
}
