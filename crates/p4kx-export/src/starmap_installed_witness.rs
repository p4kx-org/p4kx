// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;

use p4kx_extraction::{
    Archive, ArchiveWatch, Entry, InputScope, PhysicalSourceRef, SourceInput, SourceUseDescription,
    SourceUseRole, SourceUses,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_read::original::with_entry_capture;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::{CatalogError, ExportAccountingError};

#[path = "starmap_installed_oracle.rs"]
mod oracle;

#[path = "starmap_installed_hcd.rs"]
mod hcd;

#[path = "starmap_installed_entxml.rs"]
mod entxml;

#[path = "starmap_installed_source.rs"]
mod source;

use oracle::{AuthoredChild, rotation_matrix, world};
use source::{Witness, find_witness};

const POSITION_TOLERANCE_METRES: f64 = 0.001;
const ROTATION_TOLERANCE: f64 = 1.0e-10;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_starchitect_chain_matches_original_positions_and_placement_origins() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let Some(source) = find_witness(&archive, &watch)? else {
        assert!(
            std::env::var_os("P4KX_EXPECT_STARCHITECT").is_none(),
            "the installed archive must declare a structural system, body and Tile chain"
        );
        return Ok(());
    };
    let (placements, sources, system_xml, body_xml) = with_entry_capture(&archive, |capture| {
        let placements =
            Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        let sources = capture.source_uses();
        let system_xml =
            original_descriptor_ref(&archive, source.system, source.system_member, sources)?;
        let body_xml = original_descriptor_ref(&archive, source.body, source.body_member, sources)?;
        Ok((placements, sources.clone(), system_xml, body_xml))
    })?;
    let placement = placements
        .get(source.record)
        .ok_or("the authored system Child must place its selected starmap Record")?;
    close_vector(
        placement.position,
        world(source.body_route.iter())?.position,
        POSITION_TOLERANCE_METRES,
    );
    let table = placements_for(placements, &source)?;
    let inventory = sources.freeze()?;
    let usage: HashMap<_, _> = inventory
        .uses()
        .iter()
        .filter(|item| {
            matches!(
                item.role(),
                SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
            )
        })
        .map(|item| (item.id().as_str(), item))
        .collect();
    let system_route = source.body_route.iter().map(|child| (child, &system_xml));
    verify_row(&table.body, system_route.clone(), &usage)?;
    let tile_route = source.tile_route.iter().map(|child| (child, &body_xml));
    verify_row(&table.tile, system_route.chain(tile_route), &usage)?;
    watch.check()?;
    Ok(())
}

fn original_descriptor_ref(
    archive: &Archive,
    outer: &Entry,
    index: usize,
    sources: &SourceUses,
) -> Result<PhysicalSourceRef, CatalogError> {
    let container = archive
        .open_container(outer)
        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    let entry = container
        .entries()
        .get(index)
        .and_then(|entry| container.observed_entry(entry))
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?
        .source_ref()
        .clone())
}

struct SelectedRows {
    body: Value,
    tile: Value,
}

fn placements_for(mut placements: Placements, source: &Witness<'_>) -> TestResult<SelectedRows> {
    let mut body = None;
    let mut tile = None;
    let body_key = instance_key(source.system, source.body_route.iter(), &[])?;
    let tile_key = instance_key(
        source.system,
        source.body_route.iter().chain(&source.tile_route),
        &[source.body_route.len()],
    )?;
    let root_key = instance_key(source.system, std::iter::empty(), &[])?;
    for row in placements.take_instances().rows {
        let wanted_body = same_path(row.source_path_for_test(), source.body.path());
        let wanted_tile = same_path(row.source_path_for_test(), source.tile.path());
        if !(wanted_body || wanted_tile) {
            continue;
        }
        let row = serde_json::to_value(row)?;
        if wanted_body && row.pointer("/instanceId/value") == Some(&body_key) {
            assert!(body.replace(row).is_none(), "one body per physical route");
        } else if wanted_tile && row.pointer("/instanceId/value") == Some(&tile_key) {
            assert!(tile.replace(row).is_none(), "one Tile per physical route");
        }
    }
    let body = body.ok_or("original physical body route appears in placements")?;
    let tile = tile.ok_or("original physical Tile route appears in placements")?;
    assert!(
        body.pointer("/parentInstanceId/value") == Some(&root_key),
        "inline XML children retain the containing system parent"
    );
    assert!(
        tile.pointer("/parentInstanceId/value") == Some(&body_key),
        "the Tile's physical parent is the body occurrence"
    );
    Ok(SelectedRows { body, tile })
}

fn instance_key<'route>(
    root: &Entry,
    route: impl Iterator<Item = &'route AuthoredChild>,
    external_child_step_indices: &[usize],
) -> TestResult<Value> {
    let children: Vec<_> = route.map(|child| child.index).collect();
    Ok(json!({
        "rootEntryIndex": u32::try_from(root.index())?,
        "childNodeIndices": children,
        "externalChildStepIndices": external_child_step_indices,
    }))
}

fn verify_row<'route>(
    row: &Value,
    route: impl Iterator<Item = (&'route AuthoredChild, &'route PhysicalSourceRef)>,
    uses: &HashMap<&str, &SourceUseDescription>,
) -> TestResult {
    let route: Vec<_> = route.collect();
    let &(leaf, leaf_xml) = route
        .last()
        .ok_or("the original location route has a Child")?;
    let local_position = check_payload(row, "localPosition", leaf.position, 0.0)?;
    let local_rotation = check_payload(row, "localRotation", leaf.rotation, 0.0)?;
    let world = world(route.iter().map(|&(child, _)| child))?;
    let world_position = check_payload(
        row,
        "worldPosition",
        world.position,
        POSITION_TOLERANCE_METRES,
    )?;
    let world_rotation = wire_vector::<4>(row, "worldRotation")?;
    let world_rotation_cell = row.get("worldRotation").ok_or("sourced world rotation")?;
    let actual = rotation_matrix(world_rotation)?;
    for (row, expected) in actual.into_iter().zip(world.rotation) {
        close_vector(row, expected, ROTATION_TOLERANCE);
    }
    assert_attribute(local_position, uses, leaf_xml, leaf.index, "pos")?;
    assert_attribute(local_rotation, uses, leaf_xml, leaf.index, "rot")?;
    for (index, &(child, xml)) in route.iter().enumerate() {
        if child.position_present {
            assert_attribute(world_position, uses, xml, child.index, "pos")?;
        }
        if child.rotation_present {
            assert_attribute(world_rotation_cell, uses, xml, child.index, "rot")?;
            if index + 1 < route.len() {
                assert_attribute(world_position, uses, xml, child.index, "rot")?;
            }
        }
    }
    verify_encryption(
        &route,
        [local_position, local_rotation],
        [world_position, world_rotation_cell],
    )
}

fn verify_encryption(
    route: &[(&AuthoredChild, &PhysicalSourceRef)],
    local: [&Value; 2],
    world: [&Value; 2],
) -> TestResult {
    let leaf = route.last().ok_or("original Child route")?.0;
    let expected = if route
        .iter()
        .any(|(node, _)| node.xml_encrypted == Some(true))
    {
        Some(true)
    } else if route
        .iter()
        .all(|(node, _)| node.xml_encrypted == Some(false))
    {
        Some(false)
    } else {
        None
    };
    for field in world {
        assert_eq!(
            field
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            expected,
            "composed coordinates reflect every decrypted declaration on the route"
        );
    }
    let expected = leaf.xml_encrypted;
    for value in local {
        assert_eq!(
            value
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            expected,
            "the direct XML field reflects the actual leaf and enclosing archive entry"
        );
    }
    Ok(())
}

fn assert_attribute(
    value: &Value,
    uses: &HashMap<&str, &SourceUseDescription>,
    source: &PhysicalSourceRef,
    index: u32,
    attribute: &str,
) -> TestResult {
    let refs = value
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("per-field source references")?;
    let mut found = false;
    for reference in refs {
        let name = reference.as_str().ok_or("source-use reference text")?;
        let observed = uses
            .get(name)
            .ok_or("actual contributing source-use inventory member")?;
        if let p4kx_extraction::SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = observed.location()
        {
            found |= observed.source() == source
                && namespace == "xml"
                && indices.first() == Some(&u64::from(index))
                && field
                    .strip_prefix('@')
                    .is_some_and(|field| field.eq_ignore_ascii_case(attribute));
        }
    }
    assert!(
        found,
        "the value must cite its exact authored Child attribute"
    );
    Ok(())
}

fn check_payload<'row, const N: usize>(
    row: &'row Value,
    field: &str,
    expected: [f64; N],
    tolerance: f64,
) -> TestResult<&'row Value> {
    close_vector(wire_vector(row, field)?, expected, tolerance);
    row.get(field)
        .ok_or_else(|| "sourced placement field".into())
}

fn wire_vector<const N: usize>(row: &Value, field: &str) -> TestResult<[f64; N]> {
    let cell = row.get(field).ok_or("sourced placement field")?;
    assert!(
        cell.get("state").and_then(Value::as_str) == Some("present")
            && cell.get("sourceSetComplete").and_then(Value::as_bool) == Some(true),
        "the selected readable original supplies the complete placement field"
    );
    let values = cell
        .get("value")
        .and_then(Value::as_array)
        .ok_or("original placement vector")?;
    if values.len() != N {
        return Err("unexpected placement vector arity".into());
    }
    let mut result = [0.0; N];
    for (target, value) in result.iter_mut().zip(values) {
        *target = value.as_f64().ok_or("finite placement scalar")?;
    }
    Ok(result)
}

fn close_vector<const N: usize>(left: [f64; N], right: [f64; N], limit: f64) {
    assert!(
        left.into_iter()
            .zip(right)
            .all(|(a, b)| (a - b).abs() <= limit),
        "catalog coordinates match the independently composed original coordinates"
    );
}

fn same_path(left: Option<&str>, right: &str) -> bool {
    left.is_some_and(|left| left.eq_ignore_ascii_case(right))
}
