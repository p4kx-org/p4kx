// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use serde::Serialize;

use super::submesh_origins;
use crate::model_geometry_scope::ModelGeometryScope;
use crate::{FieldOrigins, FieldPath, ModelSource, SourceInventory, SourceUseRole, SourceUses};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelFieldOrigins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ModelFieldOrigins {
    pub(super) fn capture(source: &ModelSource) -> io::Result<Self> {
        let model = source.model();
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        for (level, lod) in model.lods.iter().enumerate() {
            for index in 0..lod.meshes.len() {
                let path = FieldPath::parse(&format!("/lods/{level}/meshes/{index}"))
                    .map_err(io::Error::other)?;
                submesh_origins::write_submesh_origins(
                    source,
                    level,
                    index,
                    &path,
                    &uses,
                    &mut fields,
                )?;
            }
        }
        for index in 0..model.chunks.len() {
            let path = FieldPath::parse(&format!("/chunks/{index}")).map_err(io::Error::other)?;
            write_chunk_type_origin(source, index, &path, &uses, &mut fields)?;
        }
        Ok(Self {
            field_origins: fields,
            provenance: uses.freeze().map_err(io::Error::other)?,
        })
    }
}

/// Writes the origin of the numeric type ID for a chunk at its model-wide ordinal.
///
/// # Errors
/// Returns an invalid chunk position or source evidence error.
pub fn write_chunk_type_origin(
    source: &ModelSource,
    index: usize,
    chunk_path: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    let chunk = source
        .model()
        .chunks
        .get(index)
        .ok_or_else(|| io::Error::other("the selected model chunk is unavailable"))?;
    let origin = if let Some(range) = chunk.type_id_range()
        && let Some(scope) = ModelGeometryScope::capture(uses, source, chunk.file)?
    {
        let payload = scope.word(range, "typeId", SourceUseRole::Payload)?;
        uses.origin(std::slice::from_ref(&payload), scope.complete)
    } else {
        uses.origin(&[], false)
    };
    let mut path = chunk_path.clone();
    path.push_field("type_id").map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
