// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{
    PrefabFile, PrefabInstanceSource, PrefabLinkSource, PrefabLocation, PrefabLookupRoute,
    PrefabObjectLocation,
};
use p4kx_extraction::SourceUseRole::{Conversion, Payload, Selector};
use p4kx_extraction::model::PrefabTransformInputs;

use super::sites::{PrefabAttribute, Proof, Sites};

#[derive(Default)]
pub(super) struct Route {
    pub(super) selection: Proof,
    pub(super) container: Proof,
}

pub(super) fn trace(
    sites: &mut Sites<'_, '_>,
    route: Option<&PrefabInstanceSource>,
) -> Result<Route> {
    let mut result = Route::default();
    for instance in std::iter::successors(route, |&instance| instance.parent()) {
        let selection = instance.selection();
        let object = selection.object();
        link(sites, &mut result.selection, object, selection.lookup())?;
        let source = object.source();
        sites.attribute(
            &mut result.container,
            object.prefab.file,
            source.map(|source| &source.transform.element),
            source.and_then(|source| source.name.as_ref()),
            "Name",
            Payload,
        )?;
    }
    Ok(result)
}

pub(super) fn link(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    object: PrefabObjectLocation<'_>,
    lookup: &PrefabLinkSource,
) -> Result<()> {
    reference(sites, proof, object)?;
    if lookup.searched_directory() {
        sites.survey_directory(proof)?;
    }
    for file in lookup.surveyed_names() {
        sites.file_selector(proof, file)?;
        let node = file.source().map(|source| &source.root);
        sites.element(proof, file, node, "Prefab/Name", Selector)?;
    }
    for &index in lookup.failed_entry_indices() {
        sites.failed_lookup(index)?;
    }
    if let Some(named) = lookup.compared_name() {
        name_match(sites, proof, named)?;
        sites.prefab_attribute(proof, named, PrefabAttribute::Id, Selector)?;
    }
    let route = lookup.route();
    if let Some(target) = lookup.target() {
        chosen(sites, proof, target, route)?;
    }
    if matches!(
        route,
        PrefabLookupRoute::GuidIndex
            | PrefabLookupRoute::NameFallback
            | PrefabLookupRoute::UnmatchedGuid
    ) {
        proof.complete = false;
    }
    Ok(())
}

fn reference(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    object: PrefabObjectLocation<'_>,
) -> Result<()> {
    let source = object.source();
    let node = source.map(|source| &source.transform.element);
    let reference = source.and_then(|source| source.reference.as_ref());
    for (name, attribute) in [
        ("Type", source.and_then(|source| source.kind.as_ref())),
        (
            "PrefabName",
            reference.and_then(|reference| reference.name.as_ref()),
        ),
        (
            "PrefabGUID",
            reference.and_then(|reference| reference.guid.as_ref()),
        ),
    ] {
        sites.attribute(proof, object.prefab.file, node, attribute, name, Selector)?;
    }
    Ok(())
}

fn chosen(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    target: PrefabLocation<'_>,
    route: PrefabLookupRoute,
) -> Result<()> {
    match route {
        PrefabLookupRoute::Name => name_match(sites, proof, target)?,
        PrefabLookupRoute::NameAndGuid | PrefabLookupRoute::NameFallback => {
            name_match(sites, proof, target)?;
            sites.prefab_attribute(proof, target, PrefabAttribute::Id, Selector)?;
        }
        PrefabLookupRoute::GuidIndex => {
            sites.prefab_attribute(proof, target, PrefabAttribute::Id, Selector)?;
        }
        _ => proof.complete = false,
    }
    Ok(())
}

fn name_match(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    target: PrefabLocation<'_>,
) -> Result<()> {
    sites.file_selector(proof, target.file)?;
    sites.prefab_attribute(proof, target, PrefabAttribute::Name, Selector)
}

#[derive(Clone, Copy)]
pub(super) enum TransformUse {
    Matrix,
    Translation,
    Linear,
}

pub(super) fn transforms(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    object: PrefabObjectLocation<'_>,
    mode: TransformUse,
) -> Result<()> {
    let file = object.prefab.file;
    let source = object.source();
    transform(
        sites,
        proof,
        file,
        source.map(|source| &source.transform),
        mode,
    )?;
    let first = source.and_then(|source| source.groups.as_deref());
    for group in std::iter::successors(first, |&group| group.parent.as_deref()) {
        transform(
            sites,
            proof,
            file,
            Some(&group.transform),
            match mode {
                TransformUse::Linear => TransformUse::Linear,
                TransformUse::Matrix | TransformUse::Translation => TransformUse::Matrix,
            },
        )?;
    }
    Ok(())
}

fn transform(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    file: &PrefabFile,
    input: Option<&PrefabTransformInputs>,
    mode: TransformUse,
) -> Result<()> {
    let Some(input) = input else {
        return sites.element(proof, file, None, "Transform", Conversion);
    };
    let (position_role, angular_role) = match mode {
        TransformUse::Matrix => (Conversion, Conversion),
        TransformUse::Translation => (Conversion, Selector),
        TransformUse::Linear => (Selector, Conversion),
    };
    for (name, field, role) in [
        ("Pos", input.position.as_ref(), position_role),
        ("Rotate", input.rotation.as_ref(), angular_role),
        ("Scale", input.scale.as_ref(), angular_role),
    ] {
        sites.attribute(proof, file, Some(&input.element), field, name, role)?;
    }
    Ok(())
}
