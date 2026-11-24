// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_model::{Mesh, MeshSubmeshSource};

use crate::model_geometry_scope::ModelGeometryScope;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelSource, SourceUse, SourceUseRole, SourceUses,
};

/// Writes original submesh field origins for one mesh in its model's LOD and mesh order.
///
/// Addresses are relative to the supplied mesh path in the caller's output.
///
/// # Errors
/// Returns an invalid model position or source evidence error.
pub fn write_submesh_origins(
    source: &ModelSource,
    lod_index: usize,
    mesh_index: usize,
    parent: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    let mesh = mesh_at(source, lod_index, mesh_index)?;
    write_submeshes(source, mesh, parent, uses, fields, None)?;
    Ok(())
}

/// Writes mesh triangle and submesh origins relative to the supplied output path.
///
/// # Errors
/// Returns an invalid model position or source evidence error.
pub fn write_mesh_origins(
    source: &ModelSource,
    lod_index: usize,
    mesh_index: usize,
    parent: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    write_mesh_origins_with_triangle_uses(source, lod_index, mesh_index, parent, uses, fields)?;
    Ok(())
}

pub(super) fn write_mesh_origins_with_triangle_uses(
    source: &ModelSource,
    lod_index: usize,
    mesh_index: usize,
    parent: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<Vec<SourceUse>> {
    let mesh = mesh_at(source, lod_index, mesh_index)?;
    let mut counts = Vec::new();
    let scope = write_submeshes(source, mesh, parent, uses, fields, Some(&mut counts))?;
    let inputs = match scope {
        Some(scope) if covers_mesh_indices(mesh, counts.len()) => counts
            .into_iter()
            .map(|range| scope.word(range, "indexCount", SourceUseRole::Conversion))
            .collect::<io::Result<Vec<_>>>()?,
        _ => Vec::new(),
    };
    write_triangle_origin(parent, uses, fields, &inputs)?;
    Ok(inputs)
}

pub(super) fn write_triangle_origin(
    parent: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    counts: &[SourceUse],
) -> io::Result<()> {
    let mut path = parent.clone();
    path.push_field("triangles").map_err(io::Error::other)?;
    // The count words cover emitted ranges. Geometry and row selection also affect the total.
    fields
        .insert(path, uses.origin(counts, false))
        .map_err(io::Error::other)
}

fn mesh_at(source: &ModelSource, lod_index: usize, mesh_index: usize) -> io::Result<&Mesh> {
    source
        .model()
        .lods
        .get(lod_index)
        .and_then(|lod| lod.meshes.get(mesh_index))
        .ok_or_else(|| io::Error::other("the selected model mesh is unavailable"))
}

fn covers_mesh_indices(mesh: &Mesh, observed: usize) -> bool {
    if observed == 0 || observed != mesh.submeshes.len() {
        return false;
    }
    let end = mesh.submeshes.iter().try_fold(0_usize, |start, submesh| {
        if submesh.first_index != start || !submesh.index_count.is_multiple_of(3) {
            return None;
        }
        start.checked_add(submesh.index_count)
    });
    end == Some(mesh.indices.len())
}

fn write_submeshes(
    source: &ModelSource,
    mesh: &Mesh,
    parent: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    mut counts: Option<&mut Vec<Range<u64>>>,
) -> io::Result<Option<ModelGeometryScope>> {
    let chunk = mesh.origin().and_then(|origin| {
        source
            .model()
            .chunks
            .iter()
            .find(|chunk| origin.matches_chunk(chunk))
    });
    let mut scope = None;
    let mut path = parent.clone();
    path.push_field("submeshes").map_err(io::Error::other)?;
    for (index, submesh) in mesh.submeshes.iter().enumerate() {
        let original = chunk.and_then(|chunk| mesh.source_submesh(index, chunk));
        let material = original
            .filter(|original| original.material() == submesh.material_id)
            .map(MeshSubmeshSource::material_range);
        let count = original
            .filter(|original| {
                u32::try_from(submesh.index_count).ok() == Some(original.index_count())
            })
            .map(MeshSubmeshSource::index_count_range);
        if let Some(counts) = counts.as_deref_mut()
            && let Some(range) = &count
        {
            counts.push(range.clone());
        }
        if scope.is_none()
            && (material.is_some() || count.is_some())
            && let Some(chunk) = chunk
        {
            scope = ModelGeometryScope::capture(uses, source, chunk.file)?;
        }
        path.push_index(index).map_err(io::Error::other)?;
        for (output, original, range) in [
            ("material_id", "materialId", material),
            ("index_count", "indexCount", count),
            ("first_index", "firstIndex", None),
        ] {
            path.push_field(output).map_err(io::Error::other)?;
            let origin = origin(uses, scope.as_ref().zip(range), original)?;
            fields
                .insert(path.clone(), origin)
                .map_err(io::Error::other)?;
            path.pop();
        }
        path.pop();
    }
    Ok(scope)
}

fn origin(
    uses: &SourceUses,
    word: Option<(&ModelGeometryScope, Range<u64>)>,
    field: &str,
) -> io::Result<FieldOrigin> {
    let Some((scope, range)) = word else {
        return Ok(uses.origin(&[], false));
    };
    let contribution = scope.word(range, field, SourceUseRole::Payload)?;
    Ok(uses.origin(std::slice::from_ref(&contribution), scope.complete))
}
