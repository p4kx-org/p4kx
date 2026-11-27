// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{PrefabObjectLocation, ResolvedObject};
use p4kx_extraction::SourceUseRole::{Conversion, Payload, Selector};
use p4kx_extraction::model::{PrefabAttributeObservation, PrefabModelSource, PrefabResourceEnd};
use p4kx_extraction::{FieldOrigins, SharedFieldOrigin, SourceUseRole};

use super::route::{self, TransformUse};
use super::sites::{PrefabAttribute, Proof, Sites};
use super::{entity_links, origins};

pub(super) fn collect(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    index: usize,
    object: &ResolvedObject,
) -> Result<()> {
    let path = format!("/objects/{index}");
    let location = object.location();
    let route = route::trace(sites, object.instance_route())?;
    let outer = sites
        .uses
        .shared_origin(&route.selection.uses, route.selection.complete);
    sites.put_shared(
        fields,
        &format!("{path}/container"),
        &outer,
        &route.container,
    )?;
    sites.put_shared(fields, &format!("{path}/depth"), &outer, &Proof::default())?;
    let local = if let Some(lookup) = object.unexpanded_source() {
        let mut proof = route.selection;
        route::link(sites, &mut proof, location, lookup)?;
        sites.uses.shared_origin(&proof.uses, proof.complete)
    } else {
        outer
    };
    scalar_fields(sites, fields, &path, object, &local)?;
    entity_links::collect(sites, fields, &path, object, &local)?;
    transforms(sites, fields, &path, object, &local)
}

fn scalar_fields(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    resolved: &ResolvedObject,
    shared: &SharedFieldOrigin,
) -> Result<()> {
    let object = resolved.location();
    let source = object.source();
    let file = object.prefab.file;
    let node = source.map(|source| &source.transform.element);
    let mut local = Proof::default();
    sites.element(&mut local, file, node, "Object", Selector)?;
    sites.put_shared(fields, path, shared, &local)?;
    for (path_field, name, position) in [
        (
            "name",
            "Name",
            source.and_then(|source| source.name.as_ref()),
        ),
        (
            "kind",
            "Type",
            source.and_then(|source| source.kind.as_ref()),
        ),
        ("id", "Id", source.and_then(|source| source.id.as_ref())),
        (
            "class",
            "EntityClass",
            source.and_then(|source| source.class.as_ref()),
        ),
    ] {
        let mut fact = Proof::default();
        sites.attribute(&mut fact, file, node, position, name, Payload)?;
        sites.put_shared(fields, &format!("{path}/{path_field}"), shared, &fact)?;
    }

    let mut library = Proof::default();
    library.capture(
        sites.scope_for_file(file)?,
        origins::library_name(file),
        Payload,
    )?;
    sites.put_shared(fields, &format!("{path}/library"), shared, &library)?;
    let mut prefab = Proof::default();
    sites.prefab_attribute(&mut prefab, object.prefab, PrefabAttribute::Name, Payload)?;
    sites.put_shared(fields, &format!("{path}/prefab"), shared, &prefab)?;
    let role = if resolved.model().is_some() {
        Payload
    } else {
        Selector
    };
    conditional_fields(sites, fields, path, object, shared, role)
}

fn conditional_fields(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    location: PrefabObjectLocation<'_>,
    shared: &SharedFieldOrigin,
    model_role: SourceUseRole,
) -> Result<()> {
    let mut fact = Proof::default();
    model(sites, &mut fact, location, model_role)?;
    sites.put_shared(fields, &format!("{path}/model"), shared, &fact)?;
    let mut fact = Proof::default();
    reference(sites, &mut fact, location)?;
    sites.put_shared(fields, &format!("{path}/unexpanded"), shared, &fact)?;
    let mut fact = Proof::default();
    attachment(sites, &mut fact, location)?;
    sites.put_shared(fields, &format!("{path}/attached"), shared, &fact)
}

fn model(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    location: PrefabObjectLocation<'_>,
    path_role: SourceUseRole,
) -> Result<()> {
    let file = location.prefab.file;
    let source = location.source();
    let node = source.map(|source| &source.transform.element);
    sites.attribute(
        proof,
        file,
        node,
        source.and_then(|source| source.kind.as_ref()),
        "Type",
        Selector,
    )?;
    match source.map(|source| &source.model) {
        Some(PrefabModelSource::Brush(path)) => {
            sites.attribute(proof, file, node, path.as_ref(), "Prefab", path_role)
        }
        Some(PrefabModelSource::Geometry(path)) => {
            sites.attribute(proof, file, node, path.as_ref(), "Geometry", path_role)
        }
        Some(PrefabModelSource::Resource { geometry, end }) => {
            sites.attribute(proof, file, node, geometry.as_ref(), "Geometry", Selector)?;
            match end {
                PrefabResourceEnd::Path { element, position } => sites.attribute(
                    proof,
                    file,
                    Some(element),
                    position.as_ref(),
                    "path",
                    path_role,
                ),
                PrefabResourceEnd::MissingChild { parent, child } => {
                    sites.element(proof, file, Some(parent), child.tag(), Selector)
                }
                _ => sites.element(proof, file, None, "Geometry", Selector),
            }
        }
        _ => sites.element(proof, file, None, "Geometry", Conversion),
    }
}

fn reference(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    location: PrefabObjectLocation<'_>,
) -> Result<()> {
    let file = location.prefab.file;
    let source = location.source();
    let node = source.map(|source| &source.transform.element);
    if let Some(reference) = source.and_then(|source| source.reference.as_ref()) {
        return sites.attribute(
            proof,
            file,
            node,
            reference.name.as_ref(),
            "PrefabName",
            Payload,
        );
    }
    sites.attribute(
        proof,
        file,
        node,
        source.and_then(|source| source.kind.as_ref()),
        "Type",
        Selector,
    )
}

fn attachment(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    location: PrefabObjectLocation<'_>,
) -> Result<()> {
    let file = location.prefab.file;
    let Some(source) = location.source() else {
        return sites.element(proof, file, None, "Attachment", Conversion);
    };
    let node = Some(&source.transform.element);
    let input = &source.attachment;
    sites.attribute(proof, file, node, input.parent.as_ref(), "Parent", Selector)?;
    if input.compared_group_id
        && let Some(group) = source.groups.as_deref()
    {
        sites.attribute(
            proof,
            file,
            Some(&group.transform.element),
            group.id.as_ref(),
            "Id",
            Selector,
        )?;
    }
    match &input.attachment_target {
        Some(PrefabAttributeObservation::Present(position)) => sites.attribute(
            proof,
            file,
            node,
            Some(position),
            "AttachmentTarget",
            Selector,
        ),
        Some(PrefabAttributeObservation::Absent) => {
            sites.element(proof, file, node, "AttachmentTarget", Selector)
        }
        None => Ok(()),
        _ => sites.element(proof, file, None, "AttachmentTarget", Selector),
    }
}

fn transforms(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    path: &str,
    object: &ResolvedObject,
    shared: &SharedFieldOrigin,
) -> Result<()> {
    let mut outer = Proof::default();
    let mut linear = Proof::default();
    for instance in std::iter::successors(object.instance_route(), |&instance| instance.parent()) {
        let parent = instance.selection().object();
        route::transforms(sites, &mut outer, parent, TransformUse::Matrix)?;
        route::transforms(sites, &mut linear, parent, TransformUse::Linear)?;
    }

    let location = object.location();
    let mut position = outer.clone();
    route::transforms(sites, &mut position, location, TransformUse::Translation)?;
    route::transforms(sites, &mut outer, location, TransformUse::Matrix)?;
    route::transforms(sites, &mut linear, location, TransformUse::Linear)?;

    let position = sites.shared_origin(shared, &position)?;
    let matrix = sites.shared_origin(shared, &outer)?;
    let linear = sites.shared_origin(shared, &linear)?;
    let world = object.world();
    let position_path = format!("{path}/position");
    let transform_path = format!("{path}/transform");
    Sites::put_origin(fields, &position_path, position.clone())?;
    Sites::put_origin(fields, &transform_path, matrix.clone())?;
    for (row, values) in world.rows.iter().enumerate() {
        Sites::put_origin(fields, &format!("{position_path}/{row}"), position.clone())?;
        let row_path = format!("{transform_path}/{row}");
        Sites::put_origin(fields, &row_path, matrix.clone())?;
        for (column, _) in values.iter().enumerate() {
            let origin = if column + 1 == values.len() {
                &position
            } else {
                &linear
            };
            Sites::put_origin(fields, &format!("{row_path}/{column}"), origin.clone())?;
        }
    }
    Ok(())
}
