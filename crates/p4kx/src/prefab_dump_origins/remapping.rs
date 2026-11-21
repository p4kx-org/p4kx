// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{
    PrefabGuidRemappingInstance, PrefabLocation, PrefabObjectLocation, ResolvedPrefab,
};
use p4kx_extraction::SourceUseRole::{Payload, Selector};
use p4kx_extraction::{FieldOrigins, SharedFieldOrigin};

use super::Collection;
use super::route;
use super::sites::{Proof, Sites};

pub(super) fn collect(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    resolved: &ResolvedPrefab,
) -> Result<()> {
    let mut collection = Collection::default();
    for (index, instance) in resolved.guid_remapping_instances().iter().enumerate() {
        collection.add(origins(sites, fields, index, instance)?);
    }
    if !resolved.guid_remapping_instances().is_empty() {
        sites.put(fields, "/guid_remappings", &collection.into_proof())?;
    }
    Ok(())
}

fn origins(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    index: usize,
    occurrence: &PrefabGuidRemappingInstance,
) -> Result<Proof> {
    let path = format!("/guid_remappings/{index}");
    let owner = occurrence.owner();
    let file = owner.prefab.file;
    let source = owner.source();
    let outer = route::trace(sites, occurrence.containing_route())?.selection;
    let mut row = outer.clone();
    instance_path(sites, fields, &path, occurrence, &outer)?;
    sites.element(
        &mut row,
        file,
        source.map(|source| &source.transform.element),
        "GUIDRemapping",
        Selector,
    )?;
    sites.put(fields, &format!("{path}/blocks"), &row)?;

    let shared = sites.uses.shared_origin(&row.uses, row.complete);
    blocks(sites, fields, &path, occurrence, &shared)?;

    let mut target = outer;
    if let Some(lookup) = occurrence.lookup() {
        route::link(sites, &mut target, owner, lookup)?;
    }
    target_origins(sites, fields, &path, occurrence.entered_target(), &target)?;
    row.append(target);
    sites.put(fields, &path, &row)?;
    Ok(row)
}

fn blocks(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    occurrence: &PrefabGuidRemappingInstance,
    shared: &SharedFieldOrigin,
) -> Result<()> {
    let owner = occurrence.owner();
    let file = owner.prefab.file;
    let positions = owner.source().and_then(|source| source.reference.as_ref());
    for (index, block) in occurrence.blocks().iter().enumerate() {
        let block_source =
            positions.and_then(|positions| positions.guid_remapping_blocks.get(index));
        let block_path = format!("{path}/blocks/{index}");
        let mut proof = Proof::default();
        sites.element(
            &mut proof,
            file,
            block_source.map(|source| &source.element),
            "GUIDRemapping",
            Payload,
        )?;
        sites.put_shared(fields, &block_path, shared, &proof)?;
        for (index, _) in block.iter().enumerate() {
            let pair = block_source.and_then(|source| source.pairs.get(index));
            let element = pair.map(|pair| &pair.element);
            let pair_path = format!("{block_path}/{index}");
            let mut proof = Proof::default();
            sites.element(&mut proof, file, element, "Object", Payload)?;
            sites.put_shared(fields, &pair_path, shared, &proof)?;
            for (field, name, attribute) in [
                (
                    "default_guid",
                    "defaultGUID",
                    pair.and_then(|pair| pair.default_guid.as_ref()),
                ),
                (
                    "instance_guid",
                    "instanceGUID",
                    pair.and_then(|pair| pair.instance_guid.as_ref()),
                ),
            ] {
                let mut fact = Proof::default();
                sites.attribute(&mut fact, file, element, attribute, name, Payload)?;
                sites.put_shared(fields, &format!("{pair_path}/{field}"), shared, &fact)?;
            }
        }
    }
    Ok(())
}

fn instance_path(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    occurrence: &PrefabGuidRemappingInstance,
    outer: &Proof,
) -> Result<()> {
    let path = format!("{path}/instance_path");
    let tail = occurrence.containing_route();
    let mut ancestors: Vec<_> = std::iter::successors(tail, |&instance| instance.parent())
        .map(|instance| instance.selection().object())
        .collect();
    ancestors.reverse();
    ancestors.push(occurrence.owner());
    let mut proof = outer.clone();
    for (index, location) in ancestors.into_iter().enumerate() {
        let mut selector = Proof::default();
        ordinal(sites, &mut selector, location)?;
        let mut segment = outer.clone();
        segment.append(selector.clone());
        sites.put(fields, &format!("{path}/{index}"), &segment)?;
        proof.append(selector);
    }
    sites.put(fields, &path, &proof)
}

fn ordinal(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    location: PrefabObjectLocation<'_>,
) -> Result<()> {
    sites.objects(proof, location.prefab, Selector)?;
    sites.element(
        proof,
        location.prefab.file,
        location.source().map(|source| &source.transform.element),
        "Object",
        Selector,
    )
}

fn target_origins(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    target: Option<PrefabLocation<'_>>,
    proof: &Proof,
) -> Result<()> {
    let path = format!("{path}/target");
    sites.put(fields, &path, proof)?;
    if let Some(target) = target {
        let mut file = proof.clone();
        sites.file_selector(&mut file, target.file)?;
        sites.put(fields, &format!("{path}/file"), &file)?;
        let mut ordinal = proof.clone();
        sites.element(
            &mut ordinal,
            target.file,
            target.source().map(|source| &source.element),
            "Prefab",
            Selector,
        )?;
        sites.put(fields, &format!("{path}/prefab_index"), &ordinal)?;
    }
    Ok(())
}
