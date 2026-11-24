// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_model::MeshSubmeshSource;
use serde::Serialize;

use crate::model_geometry_scope::ModelGeometryScope;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelSource, SourceInventory, SourceUseRole, SourceUses,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelSubmeshOrigins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ModelSubmeshOrigins {
    pub(super) fn capture(source: &ModelSource) -> io::Result<Self> {
        let model = source.model();
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        for (level, lod) in model.lods.iter().enumerate() {
            for index in 0..lod.meshes.len() {
                let path = FieldPath::parse(&format!("/lods/{level}/meshes/{index}"))
                    .map_err(io::Error::other)?;
                write_submesh_origins(source, level, index, &path, &uses, &mut fields)?;
            }
        }
        Ok(Self {
            field_origins: fields,
            provenance: uses.freeze().map_err(io::Error::other)?,
        })
    }
}

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
    let model = source.model();
    let mesh = model
        .lods
        .get(lod_index)
        .and_then(|lod| lod.meshes.get(mesh_index))
        .ok_or_else(|| io::Error::other("the selected model mesh is unavailable"))?;
    let chunk = mesh.origin().and_then(|origin| {
        model
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
    Ok(())
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
