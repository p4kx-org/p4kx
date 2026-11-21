// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{ChunkSource, ChunkTableEvidence, FieldOrigins, FieldPath, SourceInventory};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChunkListOrigins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ChunkListOrigins {
    pub(super) fn capture(source: &ChunkSource) -> Result<Self> {
        let evidence = ChunkTableEvidence::new(source)?;
        let rows = evidence.rows()?;
        let mut fields = FieldOrigins::new();
        fields.insert(FieldPath::parse("/file")?, evidence.path()?)?;
        fields.insert(FieldPath::parse("/container")?, evidence.container()?)?;
        fields.insert(FieldPath::parse("/version")?, evidence.version()?)?;
        fields.insert(FieldPath::parse("/chunks")?, rows.index().clone())?;
        let mut path = FieldPath::parse("/chunks")?;
        for row in 0..source.file().chunks().len() {
            path.push_index(row)?;
            rows.write_row(&mut fields, &path, row)?;
            path.pop();
        }
        Ok(Self {
            field_origins: fields,
            provenance: evidence.uses().freeze()?,
        })
    }
}
