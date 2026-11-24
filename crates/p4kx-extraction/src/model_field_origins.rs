// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use serde::Serialize;

use super::{ModelNodeOrigins, chunk_origins, submesh_origins};
use crate::{FieldOrigins, FieldPath, ModelSource, SourceInventory, SourceUses};

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
            chunk_origins::write_chunk_origins(source, index, &path, &uses, &mut fields)?;
        }
        write_nodes(source, &uses, &mut fields)?;
        Ok(Self {
            field_origins: fields,
            provenance: uses.freeze().map_err(io::Error::other)?,
        })
    }
}

fn write_nodes(
    source: &ModelSource,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    let nodes = ModelNodeOrigins::capture(source, uses)?;
    let mut path = FieldPath::parse("/nodes").map_err(io::Error::other)?;
    fields
        .insert(path.clone(), nodes.collection_origin())
        .map_err(io::Error::other)?;
    for index in 0..source.model().nodes.len() {
        path.push_index(index).map_err(io::Error::other)?;
        nodes.write_node(fields, &path, index)?;
        path.pop();
    }
    Ok(())
}
