// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_model::{Lod, Mesh, Model, ModelChunk, ModelFile, ModelIssue};

use super::source_chunk;
use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{CancelToken, FieldOrigin, ModelSource, SourceUse, SourceUseRole, SourceUses};

/// Selection evidence for the topology of the decoded model.
#[derive(Debug)]
#[non_exhaustive]
pub struct ModelTopologyOrigins {
    /// The companion name or the in-file geometry.
    pub geometry: FieldOrigin,
    /// The total count of emitted LOD rows.
    pub lod_count: FieldOrigin,
    /// Mesh counts in the emitted LOD row order.
    pub meshes_by_lod: Vec<FieldOrigin>,
}

impl ModelTopologyOrigins {
    pub(super) fn capture(
        source: &ModelSource,
        uses: &SourceUses,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        let model = source.model();
        let mut in_file_geometry = None;
        let mut lod_uses = Vec::new();
        let mut meshes_by_lod = Vec::with_capacity(model.lods.len());
        for lod in &model.lods {
            if cancel.is_cancelled() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let owner = lod_geometry_owner(lod, model);
            let mut selected_mesh = None;
            if let Some(scope) = ModelGeometryScope::capture(uses, source, owner)? {
                lod_uses.push(scope.directory_path_selector()?);
                selected_mesh = mesh_selector(source, lod, owner, &scope)?;
                if selected_mesh.is_none() {
                    lod_uses.push(scope.chunk_header(ChunkHeaderField::Signature)?);
                }
            }
            let origin = uses.origin(selected_mesh.as_slice(), false);
            if owner == ModelFile::Model && selected_mesh.is_some() {
                in_file_geometry = Some(origin.clone());
            }
            lod_uses.extend(selected_mesh);
            meshes_by_lod.push(origin);
        }
        Ok(Self {
            geometry: in_file_geometry.map_or_else(|| companion_origin(source, uses), Ok)?,
            lod_count: uses.origin(&lod_uses, false),
            meshes_by_lod,
        })
    }
}

fn lod_geometry_owner(lod: &Lod, model: &Model) -> ModelFile {
    if let Some(origin) = lod.meshes.first().and_then(Mesh::origin) {
        return origin.file;
    }
    let fallback = model
        .issues
        .iter()
        .any(|issue| matches!(issue, ModelIssue::LodFallback { level, .. } if *level == lod.level));
    if lod.level != 0 && !fallback {
        ModelFile::LodGeometry(lod.level)
    } else if model.geometry.is_some() {
        ModelFile::Geometry
    } else {
        ModelFile::Model
    }
}

fn companion_origin(source: &ModelSource, uses: &SourceUses) -> io::Result<FieldOrigin> {
    let mut selected = Vec::new();
    if source.model().geometry.is_some()
        && let Some(companion) = ModelGeometryScope::capture(uses, source, ModelFile::Geometry)?
    {
        if let Some(primary) = ModelGeometryScope::capture(uses, source, ModelFile::Model)? {
            selected.push(primary.directory_path(SourceUseRole::Conversion)?);
        }
        selected.push(companion.directory_path_selector()?);
        selected.push(companion.chunk_header(ChunkHeaderField::Signature)?);
    }
    Ok(uses.origin(&selected, false))
}

fn mesh_selector(
    source: &ModelSource,
    lod: &Lod,
    owner: ModelFile,
    scope: &ModelGeometryScope,
) -> io::Result<Option<SourceUse>> {
    // A single mesh selector witnesses the selected payload, without enumerating mesh inputs.
    let Some(mesh) = lod.meshes.first() else {
        return Ok(None);
    };
    if owner == ModelFile::Model
        && let Some(physics) = mesh.crch_physics_source(&source.model().chunks)
    {
        return scope
            .word(
                physics.primitive_range(),
                "primitive",
                SourceUseRole::Selector,
            )
            .map(Some);
    }
    source_chunk(source, mesh)
        .filter(|chunk| chunk.file == owner)
        .and_then(ModelChunk::type_id_range)
        .map(|range| scope.word(range, "typeId", SourceUseRole::Selector))
        .transpose()
}
