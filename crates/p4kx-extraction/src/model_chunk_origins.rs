// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelSource, SourceUse, SourceUseRole, SourceUses,
};

/// Writes the chunk type and physical table ordinal origins at the output path.
///
/// # Errors
/// Returns an invalid chunk position or source evidence error.
pub fn write_chunk_origins(
    source: &ModelSource,
    index: usize,
    chunk_path: &FieldPath,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    let origins = match ChunkTypeEvidence::capture(source, index, uses)? {
        Some(evidence) => evidence.origins(uses)?,
        None => std::array::from_fn(|_| uses.origin(&[], false)),
    };
    for (name, origin) in ["type_id", "type", "index"].into_iter().zip(origins) {
        insert_origin(fields, chunk_path, name, origin)?;
    }
    Ok(())
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
    let origin = ChunkTypeEvidence::capture(source, index, uses)?.map_or_else(
        || uses.origin(&[], false),
        |evidence| evidence.type_id(uses),
    );
    insert_origin(fields, chunk_path, "type_id", origin)
}

struct ChunkTypeEvidence {
    scope: ModelGeometryScope,
    kind: SourceUse,
}

impl ChunkTypeEvidence {
    fn capture(source: &ModelSource, index: usize, uses: &SourceUses) -> io::Result<Option<Self>> {
        let chunk = source
            .model()
            .chunks
            .get(index)
            .ok_or_else(|| io::Error::other("the selected model chunk is unavailable"))?;
        let Some(range) = chunk.type_id_range() else {
            return Ok(None);
        };
        let Some(scope) = ModelGeometryScope::capture(uses, source, chunk.file)? else {
            return Ok(None);
        };
        let kind = scope.word(range, "typeId", SourceUseRole::Payload)?;
        Ok(Some(Self { scope, kind }))
    }

    fn type_id(&self, uses: &SourceUses) -> FieldOrigin {
        uses.origin(std::slice::from_ref(&self.kind), self.scope.complete)
    }

    fn origins(self, uses: &SourceUses) -> io::Result<[FieldOrigin; 3]> {
        let numeric = self.type_id(uses);
        let signature = self.scope.chunk_header(ChunkHeaderField::Signature)?;
        let table_offset = self.scope.chunk_header(ChunkHeaderField::TableOffset)?;
        let count = self.scope.chunk_header(ChunkHeaderField::RowCount)?;
        Ok([
            numeric,
            uses.origin(&[self.kind, signature], self.scope.complete),
            uses.origin(&[table_offset, count], self.scope.complete),
        ])
    }
}

fn insert_origin(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let mut path = parent.clone();
    path.push_field(name).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
