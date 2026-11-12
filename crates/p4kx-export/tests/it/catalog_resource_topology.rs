// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_resource_topology_fixture.rs"]
mod fixture;

#[test]
fn resource_topology_preserves_the_original_hardpoint_weak_and_room_occurrences() -> TestResult {
    let catalog = catalog()?;
    let links = owner(
        &catalog,
        "/declarations/resourceNetworkLinks/rows",
        fixture::SHIP,
    )?;
    let hardpoint = proof::rows(links, "/internalHardpointLinks/value")?;
    assert_eq!(
        hardpoint.len(),
        7,
        "retain cyclic and repeated declarations"
    );
    for (ordinal, row) in hardpoint.iter().enumerate() {
        proof::present(
            proof::at(row, "/value")?,
            "linkOrdinal",
            &json!(ordinal),
            Some(true),
        )?;
    }
    proof::present(
        proof::at(&hardpoint[0], "/value")?,
        "port1",
        &json!("mount_a"),
        Some(true),
    )?;
    proof::present(
        proof::at(&hardpoint[3], "/value")?,
        "port1",
        &json!("mount_a"),
        Some(true),
    )?;
    let weak = proof::rows(links, "/internalResourceLinks/value")?;
    assert_eq!(weak.len(), 1, "weak edges are a separate authored family");
    let first = proof::at(&weak[0], "/value/port1/value/target")?;
    assert_eq!(first["type"], "instance", "an original owned port target");
    proof::present(
        first,
        "location",
        &json!("Components[1].Ports[5]"),
        Some(true),
    )?;
    let rooms = owner(
        &catalog,
        "/declarations/roomAssociations/rows",
        fixture::SHIP,
    )?;
    let originals = proof::rows(rooms, "/objectContainers/value")?;
    assert_eq!(originals.len(), 2, "include the unresolvable declaration");
    let room = proof::at(&originals[0], "/value")?;
    for (field, word) in [
        ("resourceNetworkItemportName", "mount_a"),
        ("boneName", "bridge"),
        ("fileName", "data/fixture/room.socpak"),
        ("guid", "fixture-room-id"),
    ] {
        proof::present(room, field, &json!(word), Some(true))?;
    }
    proof::closure(&catalog)?;
    Ok(())
}

fn catalog() -> TestResult<Value> {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    catalog_from(&archive)
}

fn catalog_from(archive: &p4kx_extraction::Archive) -> TestResult<Value> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn owner<'catalog>(
    catalog: &'catalog Value,
    path: &str,
    guid: u128,
) -> TestResult<&'catalog Value> {
    let guid = common::guid(guid);
    proof::rows(catalog, path)?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or_else(|| format!("missing {guid} at {path}").into())
}

#[test]
fn topology_binds_each_original_in_the_declaring_loadout_context() -> TestResult {
    let (_directory, archive) = fixture::graph_archive()?;
    let catalog = catalog_from(&archive)?;
    let (root, ship) = loadout(&catalog, fixture::SHIP)?;
    check_links(&catalog, root, ship)?;
    check_nodes(&catalog, root, ship)?;
    check_rooms(&catalog, root)?;
    check_restrictions(&catalog)?;
    proof::closure(&catalog)?;
    Ok(())
}

#[test]
fn unread_vehicle_inputs_leave_observed_candidates_without_claiming_absence() -> TestResult {
    for (origin, encryption) in [
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Plain, Value::Null),
    ] {
        let (_directory, archive) = common::open(&fixture::database(), origin)?;
        let catalog = catalog_from(&archive)?;
        let (root, _) = loadout(&catalog, fixture::SHIP)?;
        let links = bindings(&catalog, root)?;
        let first = proof::at(links.first().ok_or("first hardpoint link")?, "/port1")?;
        assert_eq!(
            first["containsEncryptedSource"], encryption,
            "the partial contributor set keeps only proven encryption"
        );
        assert_eq!(
            first["sourceSetComplete"], false,
            "the original vehicle scope is unreadable"
        );
        assert_eq!(
            first["value"]["state"], "unavailable",
            "the one observed candidate has incomplete context"
        );
        assert_eq!(
            proof::rows(first, "/value/candidates")?.len(),
            1,
            "retain the observed candidate"
        );
        assert_eq!(
            proof::at(
                links.get(4).ok_or("unmatched hardpoint link")?,
                "/port2/value/state"
            )?,
            "unavailable",
            "unread input prevents a negative assertion"
        );
        let weak = links
            .into_iter()
            .find(|row| row["family"] == "resource")
            .ok_or("weak link")?;
        assert_eq!(
            proof::at(weak, "/port1/value/state")?,
            "unavailable",
            "unread input prevents claiming a final weak binding"
        );
        assert_eq!(proof::rows(weak, "/port1/value/candidates")?.len(), 1);
    }
    Ok(())
}

fn check_links(catalog: &Value, root: usize, ship: &Value) -> TestResult {
    let root_links = bindings(catalog, root)?;
    let hardpoints: Vec<_> = root_links
        .iter()
        .copied()
        .filter(|row| row["family"] == "hardpoint")
        .collect();
    assert_eq!(
        hardpoints.len(),
        9,
        "seven root links and one for each mounted relay"
    );
    let first = hardpoints.first().ok_or("first hardpoint link")?;
    let ambiguous = hardpoints.get(4).ok_or("ambiguous hardpoint link")?;
    assert_eq!(
        proof::at(first, "/port1/value/state")?,
        "resolved",
        "known named endpoint"
    );
    assert_eq!(
        proof::at(first, "/port2/value/state")?,
        "resolved",
        "known non-relay endpoint"
    );
    assert_eq!(
        proof::at(ambiguous, "/port1/value/state")?,
        "ambiguous",
        "duplicate authored name"
    );
    assert_eq!(
        proof::rows(ambiguous, "/port1/value/candidates")?.len(),
        2,
        "both physical definitions survive"
    );
    assert_eq!(
        proof::at(ambiguous, "/port2/value/state")?,
        "missing",
        "unknown name in a complete context"
    );
    let weak = root_links
        .iter()
        .copied()
        .find(|row| row["family"] == "resource")
        .ok_or("weak link")?;
    assert_eq!(
        proof::at(weak, "/port1/value/state")?,
        "resolved",
        "the physical pointer survives name ambiguity"
    );
    for mount in ["mount_a", "mount_b"] {
        let path = vec![named_port_ordinal(ship, mount)?];
        assert!(
            hardpoints
                .iter()
                .any(|row| row.pointer("/context/portOrdinals") == Some(&json!(path))),
            "each repeated relay has a distinct owner context"
        );
    }
    check_context_boundaries(catalog, ship, &hardpoints)
}

fn check_context_boundaries(catalog: &Value, ship: &Value, hardpoints: &[&Value]) -> TestResult {
    let docked = named_port_ordinal(ship, "hangar")?;
    assert!(
        !hardpoints
            .iter()
            .any(|row| row.pointer("/context/portOrdinals") == Some(&json!([docked]))),
        "docked graph remains outside enclosing context"
    );
    let (docked_root, _) = loadout(catalog, fixture::DOCKED)?;
    assert_eq!(
        bindings(catalog, docked_root)?.len(),
        1,
        "docked vehicle keeps its own graph"
    );
    Ok(())
}

fn check_nodes(catalog: &Value, root: usize, ship: &Value) -> TestResult {
    let nodes = proof::rows(catalog, "/placements/resourceNetworkNodes/rows")?;
    for (name, item, relay) in [
        ("mount_a", fixture::RELAY, true),
        ("mount_b", fixture::RELAY, true),
        ("loose", fixture::ACTOR, false),
    ] {
        let node = placed_node(nodes, root, &[named_port_ordinal(ship, name)?])?;
        let evidence = proof::at(node, "/resourceComponents/0")?;
        proof::present(evidence, "isRelay", &json!(relay), Some(true))?;
        proof::present(
            proof::at(node, "/attachedItem")?,
            "targetGuid",
            &json!(common::guid(item)),
            Some(true),
        )?;
        assert_eq!(
            node["fuseEvidence"], "unestablished",
            "class names are not fuse evidence"
        );
    }
    let mount = crate::loadouts_support::port(&ship["ports"], "mount_a")?;
    assert_eq!(
        mount["visibility"], "invisible",
        "invisible placements remain graph nodes"
    );
    let docked = named_port_ordinal(ship, "hangar")?;
    let boundary = placed_node(nodes, root, &[docked])?;
    proof::present(boundary, "vehicleBoundary", &json!(true), Some(true))?;
    Ok(())
}

fn check_rooms(catalog: &Value, root: usize) -> TestResult {
    let rooms = proof::rows(catalog, "/placements/roomAssociationBindings/rows")?;
    let root_rooms: Vec<_> = rooms
        .iter()
        .filter(|row| row.pointer("/context/loadoutOrdinal") == Some(&json!(root)))
        .collect();
    let [matched, unmatched] = root_rooms.as_slice() else {
        return Err("both authored room associations remain".into());
    };
    assert_eq!(
        proof::at(matched, "/port/value/state")?,
        "resolved",
        "stored room selector"
    );
    assert_eq!(
        proof::at(unmatched, "/port/value/state")?,
        "missing",
        "unmatched room selector remains visible"
    );
    Ok(())
}

fn check_restrictions(catalog: &Value) -> TestResult {
    let ports = owner(catalog, "/configurations/itemPorts/rows", fixture::SHIP)?;
    let mounted = proof::at(ports, "/ports/value/0/value/restrictions")?;
    proof::present(mounted, "resourceLinkToParent", &json!(true), Some(true))?;
    proof::present(
        mounted,
        "requiredPortTags",
        &json!("fixture-required"),
        Some(true),
    )?;
    proof::present(
        mounted,
        "minSize",
        &json!({"type": "int32", "value": 1}),
        Some(true),
    )?;
    let accepted = proof::at(mounted, "/types/value/0/value")?;
    proof::present(accepted, "type", &json!("FixtureDevice"), Some(true))?;
    proof::cell(accepted, "subTypes", "present")?;
    let item = owner(
        catalog,
        "/configurations/itemAttachmentConstraints/rows",
        fixture::RELAY,
    )?;
    proof::present(
        proof::at(item, "/definition/value")?,
        "itemType",
        &json!("Misc"),
        Some(true),
    )?;
    Ok(())
}

fn loadout(catalog: &Value, guid: u128) -> TestResult<(usize, &Value)> {
    let entry = crate::loadouts_support::loadout(catalog, guid)?;
    let ordinal = proof::rows(catalog, "/loadouts")?
        .iter()
        .position(|row| std::ptr::eq(row, entry))
        .ok_or("ship Loadout position")?;
    Ok((ordinal, entry))
}

fn bindings(catalog: &Value, loadout: usize) -> TestResult<Vec<&Value>> {
    Ok(
        proof::rows(catalog, "/placements/resourceNetworkLinkBindings/rows")?
            .iter()
            .filter(|row| row.pointer("/context/loadoutOrdinal") == Some(&json!(loadout)))
            .collect(),
    )
}

fn named_port_ordinal(ship: &Value, name: &str) -> TestResult<usize> {
    proof::rows(ship, "/ports")?
        .iter()
        .position(|port| port["name"] == name)
        .ok_or_else(|| format!("no loadout port {name}").into())
}

fn placed_node<'rows>(
    nodes: &'rows [Value],
    loadout: usize,
    ordinals: &[usize],
) -> TestResult<&'rows Value> {
    nodes
        .iter()
        .find(|node| {
            node.pointer("/position/loadoutOrdinal") == Some(&json!(loadout))
                && node.pointer("/position/portOrdinals") == Some(&json!(ordinals))
        })
        .ok_or_else(|| format!("no topology node for {loadout}/{ordinals:?}").into())
}

#[path = "catalog_resource_topology_origin.rs"]
mod origin;
