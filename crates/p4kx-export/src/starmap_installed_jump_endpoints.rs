// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::io::{Write, stdout};
use std::time::UNIX_EPOCH;

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, Filter, Guid, TreeBuilder, Value};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

use super::jump_class::JumpEndpointRecords;
use super::{Limits, Placements};
use crate::build::original_input::test_support::{TestResult, with_archive_input};
use crate::catalog_read::original::EntryCapture;
use crate::starmap_index::Index;
use crate::starmap_parse::installed::read_jump_witness_body;
use crate::starmap_parse::{Body, Level};
use crate::starmap_walk::System;
use crate::{CatalogError, CatalogOptions};

const COMPONENT: &str = "SJumpPointParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_jump_endpoint_routes_resolve_original_operating_components() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let filter = Filter::new(
        &std::env::var("P4KX_JUMP_ENDPOINT_FILTER").unwrap_or_else(|_| "*jump*".to_owned()),
    );
    with_archive_input(&archive, |forge, input| {
        let options = CatalogOptions {
            filter: Some(&filter),
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let candidates = JumpEndpointRecords::read(forge, input, &options)?;
        let capture = EntryCapture::for_input(input);
        let entities = Entities::new(forge);
        let mut placements = read_roots(&archive, &capture, &candidates)?;
        let owners = placements.join_jump_operators(&entities, input, None)?;
        let table = placements.take_jump_endpoints();
        let verified = table.rows.iter().try_fold(0, |count, row| {
            let json = serde_json::to_value(row)?;
            verify_endpoint(forge, &owners, &json).map(|added| count + added)
        })?;
        assert!(
            verified > 0,
            "the selected build must prove an operational endpoint"
        );
        writeln!(
            stdout().lock(),
            "bounded witness read {} routes and verified {verified} local operating links",
            table.rows.len()
        )?;
        Ok(())
    })?;
    watch.check()?;
    Ok(())
}

fn read_roots(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    candidates: &JumpEndpointRecords,
) -> Result<Placements, CatalogError> {
    let index = Index::new(archive);
    let mut placements = Placements::default();
    for (system, root) in index.systems() {
        let body = read_jump_witness_body(archive, &index, root, capture)?;
        let Body::Parsed { root: tree, .. } = &body else {
            continue;
        };
        let targets = direct_targets(tree, candidates);
        let mut bodies = HashMap::new();
        for path in targets {
            if let [target] = index.endpoint_candidates(path) {
                bodies.insert(
                    *target,
                    read_jump_witness_body(archive, &index, *target, capture)?,
                );
            }
        }
        bodies.insert(root, body);
        let read = System::run(
            &index,
            &bodies,
            &system,
            root,
            None,
            Limits::default(),
            (archive, capture, Some(candidates)),
        )?;
        placements.jump_endpoints.extend(read.jump_endpoints);
    }
    Ok(placements)
}

fn direct_targets<'source>(
    root: &'source Level,
    candidates: &JumpEndpointRecords,
) -> HashSet<&'source str> {
    let mut paths = HashSet::new();
    let mut pending = vec![root];
    while let Some(level) = pending.pop() {
        for child in &level.children {
            if child.record.and_then(|guid| candidates.get(guid)).is_some()
                && let Some(path) = &child.external
            {
                paths.insert(path.as_str());
            }
            pending.push(&child.below);
        }
    }
    paths
}

fn verify_endpoint(forge: &DataForge<'_>, owners: &HashSet<u32>, row: &Json) -> TestResult<usize> {
    if row
        .pointer("/gate/value/marker/value")
        .and_then(Json::as_bool)
        != Some(true)
    {
        return Ok(0);
    }
    let entities = row
        .pointer("/gate/value/entities")
        .and_then(Json::as_array)
        .ok_or("typed metadata Entities")?;
    assert!(
        !entities.is_empty(),
        "the scoped marked gate has a companion with native Entities"
    );
    for entity in entities {
        verify_entity(forge, owners, entity)?;
    }
    assert_eq!(
        row.pointer("/declaredPartner/state").and_then(Json::as_str),
        Some("absent"),
        "the supported and readable local source does not declare a partner"
    );
    Ok(entities.len())
}

fn verify_entity(forge: &DataForge<'_>, owners: &HashSet<u32>, entity: &Json) -> TestResult {
    assert_eq!(
        entity.pointer("/object/state").and_then(Json::as_str),
        Some("present"),
        "unique physical editor object after the native metadata CryGUID"
    );
    let class = entity
        .pointer("/matches/value/0/class/value")
        .and_then(Json::as_str)
        .ok_or("authored class")?;
    let cry_guid = entity
        .pointer("/cryGuid/value")
        .and_then(Json::as_str)
        .ok_or("authored CryGUID")?
        .parse::<Guid>()?;
    let editor_guid = entity
        .pointer("/matches/value/0/guid/value")
        .and_then(Json::as_str)
        .ok_or("physical editor GUID")?
        .parse::<Guid>()?;
    assert_eq!(
        cry_guid, editor_guid,
        "exact GUID names only the local editor object"
    );
    let owner = entity
        .pointer("/operatingEntity/value/ownerRecordIndex")
        .and_then(Json::as_u64)
        .map(u32::try_from)
        .transpose()?
        .ok_or("physical operating Entity Record")?;
    let owner_guid = entity
        .pointer("/operatingEntity/value/ownerGuid")
        .and_then(Json::as_str)
        .ok_or("owner GUID")?
        .parse::<Guid>()?;
    let record = forge.record(owner).ok_or("exact original Record")?;
    assert_eq!(record.guid(), owner_guid, "physical header GUID");
    assert_eq!(
        record.name()?,
        format!("EntityClassDefinition.{class}"),
        "authored editor class points to exact DCB class"
    );
    assert_eq!(
        record.type_name()?,
        "EntityClassDefinition",
        "original owner type"
    );
    assert!(
        owners.contains(&owner),
        "the linked owner is selected for the configuration table"
    );
    verify_component_positions(entity, &record)
}

fn verify_component_positions(entity: &Json, record: &p4kx_extraction::Record<'_>) -> TestResult {
    let actual = entity
        .pointer("/operatingConfigurations/value")
        .and_then(Json::as_array)
        .ok_or("operating configuration references")?;
    let read = TreeBuilder::new().build(record)?;
    let Some(Value::Array(components)) = read.field("Components") else {
        return Err("original Entity Components array".into());
    };
    let positions: Vec<_> = components
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            matches!(value, Value::Struct(component) if component.type_name() == COMPONENT)
                .then_some(index)
        })
        .collect();
    assert!(
        !positions.is_empty(),
        "the installed source has an original operating component"
    );
    assert_eq!(
        positions.len(),
        actual.len(),
        "every original slot appears once"
    );
    for (&original, row) in positions.iter().zip(actual) {
        assert_eq!(
            row["ownerRecordIndex"],
            record.index(),
            "original Record identity"
        );
        assert_eq!(
            row["componentOrdinal"], original,
            "original Components slot identity"
        );
    }
    Ok(())
}
