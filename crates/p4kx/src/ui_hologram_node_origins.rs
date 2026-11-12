// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ops::Range;

use anyhow::Result;
use p4kx_assembly::Assembly;
use p4kx_extraction::model::{Model, ModelFile, ModelNodeSource};
use p4kx_extraction::{
    InputScope, ModelArchiveSource, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole,
};

use p4kx_render::MeshNodeInput;

use super::{Projection, cdf_origins};

pub(super) fn placement(
    projection: &Projection,
    assembly: &Assembly,
    part: usize,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    for source in assembly.model_placement_inputs(part) {
        projection.authored_selector(source.geometry_selector(), uses)?;
        if let Some(definition) = source.definition_origin() {
            uses.push(projection.opaque_archive(definition, SourceUseRole::Selector)?);
            cdf_origins::model_selectors(
                projection,
                Some(definition),
                source.cdf_model_selectors(),
                uses,
            )?;
        }
        let scope = projection
            .uses()
            .observe(SourceInput::Archive(source.origin()), InputScope::Payload)?;
        lineage(&scope, Route::Placement, source.nodes(), uses)?;
    }
    Ok(())
}

pub(super) fn mesh(
    projection: &Projection,
    model: Option<&Model>,
    archive: &[ModelArchiveSource],
    inputs: impl Iterator<Item = MeshNodeInput>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(model) = model else {
        return Ok(());
    };
    let Some(primary) = archive
        .iter()
        .find(|source| source.file() == ModelFile::Model)
    else {
        return Ok(());
    };
    let Some((origin, nodes)) = primary.origin().zip(primary.node_sources()) else {
        return Ok(());
    };
    let mut inputs = inputs.peekable();
    if inputs.peek().is_none() {
        return Ok(());
    }
    let scope = projection
        .uses()
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let mut seen = HashSet::new();
    for mesh in inputs.filter(|input| seen.insert(*input)) {
        lineage(
            &scope,
            Route::Mesh(mesh),
            nodes.lineage(model, mesh.node()),
            uses,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Route {
    Placement,
    Mesh(MeshNodeInput),
}

fn lineage<'a>(
    scope: &SourceScope,
    route: Route,
    nodes: impl Iterator<Item = (usize, &'a ModelNodeSource)>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    for (step, (ordinal, node)) in nodes.enumerate() {
        let selected = if step == 0 {
            match route {
                Route::Placement => node.name().map(|name| ("name", name)),
                Route::Mesh(_) => None,
            }
        } else {
            node.id().map(|id| ("chunkId", id))
        };
        let coordinates = (u64::try_from(node.chunk())?, u64::try_from(ordinal)?);
        if let Some((field, range)) = selected {
            uses.push(at(
                scope,
                route,
                coordinates,
                field,
                range,
                SourceUseRole::Selector,
            )?);
        }
        uses.push(at(
            scope,
            route,
            coordinates,
            "parent",
            node.parent(),
            SourceUseRole::Selector,
        )?);
        uses.push(at(
            scope,
            route,
            coordinates,
            "localTransform",
            node.matrix(),
            SourceUseRole::Conversion,
        )?);
    }
    Ok(())
}

fn at(
    scope: &SourceScope,
    route: Route,
    (chunk, node): (u64, u64),
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse> {
    let (namespace, indices) = match route {
        Route::Placement => (
            "modelNodePlacementBytes",
            vec![chunk, node, range.start, range.end],
        ),
        Route::Mesh(mesh) => (
            "modelMeshNodeBytes",
            vec![
                u64::from(mesh.level()),
                u64::try_from(mesh.mesh())?,
                chunk,
                node,
                range.start,
                range.end,
            ],
        ),
    };
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        },
        role,
    )?)
}
