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
        5,
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
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
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
