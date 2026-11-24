// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use serde::Serialize;

use super::{chunk_origins, submesh_origins};
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
        Ok(Self {
            field_origins: fields,
            provenance: uses.freeze().map_err(io::Error::other)?,
        })
    }
}
