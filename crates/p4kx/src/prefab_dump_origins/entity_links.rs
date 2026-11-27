// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_assembly::{PrefabFile, ResolvedObject};
use p4kx_extraction::SourceUseRole::{Conversion, Payload, Selector};
use p4kx_extraction::model::{
    PrefabEntityLink, PrefabEntityLinkGroup, PrefabEntityLinkGroupSource, PrefabEntityLinkSource,
};
use p4kx_extraction::{FieldOrigins, SharedFieldOrigin};

use super::sites::{Proof, Sites};

pub(super) fn collect(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    object_path: &str,
    object: &ResolvedObject,
    route: &SharedFieldOrigin,
) -> Result<()> {
    let groups = object.entity_links();
    if groups.is_empty() {
        return Ok(());
    }
    let location = object.location();
    let file = location.prefab.file;
    let object_source = location.source();
    let source = object_source.and_then(|source| source.entity_links.as_deref());
    ensure!(
        object_source.is_none() || source.is_some_and(|source| source.groups.len() == groups.len()),
        "Prefab entity-link group sources do not match rows"
    );

    let path = format!("{object_path}/entity_links");
    let mut collection = Proof {
        complete: source.is_some_and(|source| !source.has_other_children),
        ..Proof::default()
    };
    sites.element(
        &mut collection,
        file,
        object_source.map(|source| &source.transform.element),
        "EntityLinks",
        Selector,
    )?;
    for (index, group) in groups.iter().enumerate() {
        let position = source.and_then(|source| source.groups.get(index));
        collection.append(collect_group(
            sites,
            fields,
            &format!("{path}/{index}"),
            file,
            group,
            position,
            route,
        )?);
    }
    sites.put_shared(fields, &path, route, &collection)
}

fn collect_group(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    file: &PrefabFile,
    group: &PrefabEntityLinkGroup,
    source: Option<&PrefabEntityLinkGroupSource>,
    route: &SharedFieldOrigin,
) -> Result<Proof> {
    ensure!(
        source.is_none_or(|source| source.links.len() == group.links.len()),
        "Prefab entity-link sources do not match rows"
    );
    let mut proof = Proof {
        complete: source.is_some_and(|source| !source.has_other_children),
        ..Proof::default()
    };
    sites.element(
        &mut proof,
        file,
        source.map(|source| &source.element),
        "EntityLinks",
        Payload,
    )?;
    let links_path = format!("{path}/links");
    for (index, link) in group.links.iter().enumerate() {
        let source = source.and_then(|source| source.links.get(index));
        proof.append(collect_link(
            sites,
            fields,
            &format!("{links_path}/{index}"),
            file,
            link,
            source,
            route,
        )?);
    }
    sites.put_shared(fields, &links_path, route, &proof)?;
    sites.put_shared(fields, path, route, &proof)?;
    Ok(proof)
}

fn collect_link(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    file: &PrefabFile,
    link: &PrefabEntityLink,
    source: Option<&PrefabEntityLinkSource>,
    route: &SharedFieldOrigin,
) -> Result<Proof> {
    let element = source.map(|source| &source.element);
    let mut row = Proof::default();
    sites.element(&mut row, file, element, "Link", Payload)?;
    sites.put_shared(fields, path, route, &row)?;
    for (field, attribute, authored, position) in [
        (
            "name",
            "Name",
            link.name.as_ref(),
            source.and_then(|source| source.name.as_ref()),
        ),
        (
            "target_id",
            "TargetId",
            link.target_id.as_ref(),
            source.and_then(|source| source.target_id.as_ref()),
        ),
        (
            "version",
            "Version",
            link.version.as_ref(),
            source.and_then(|source| source.version.as_ref()),
        ),
    ] {
        let mut proof = Proof::default();
        if source.is_some() {
            sites.attribute(&mut proof, file, element, position, attribute, Payload)?;
        } else {
            let role = if authored.is_some() {
                Payload
            } else {
                Conversion
            };
            sites.element(&mut proof, file, None, attribute, role)?;
        }
        sites.put_shared(fields, &format!("{path}/{field}"), route, &proof)?;
    }
    Ok(row)
}
