// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_model::ModelFile;

use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelSource, SourceUse, SourceUseRole, SourceUses,
};

/// Reports the original chunk counts of the model and its admitted companion files.
///
/// # Errors
/// Returns a source evidence error.
pub fn model_chunk_count_origin(
    source: &ModelSource,
    uses: &SourceUses,
) -> io::Result<FieldOrigin> {
    let mut contributors = Vec::new();
    for read in source.archive_sources() {
        let Some(scope) = ModelGeometryScope::from_model_read(uses, read)? else {
            continue;
        };
        contributors.push(scope.directory_path_selector()?);
        for field in [
            ChunkHeaderField::Signature,
            ChunkHeaderField::Version,
            ChunkHeaderField::TableOffset,
        ] {
            contributors.push(
                field
                    .observe(scope.scope(), SourceUseRole::Selector)
                    .map_err(io::Error::other)?,
            );
        }
        contributors.push(
            ChunkHeaderField::RowCount
                .observe(scope.scope(), SourceUseRole::Conversion)
                .map_err(io::Error::other)?,
        );
    }
    Ok(uses.origin(&contributors, false))
}

/// Immediate origins of one emitted chunk summary.
#[derive(Debug)]
#[non_exhaustive]
pub struct ModelChunkOrigins {
    /// Numeric chunk type.
    pub type_id: FieldOrigin,
    /// Name or absence of a known chunk type.
    pub kind: FieldOrigin,
    /// Position in the source file's chunk table.
    pub index: FieldOrigin,
    /// Role of the physical model or geometry file.
    pub file: FieldOrigin,
    /// Classification by the model reader.
    pub usage: FieldOrigin,
}

/// Captures one chunk summary from the admitted model and its physical file.
///
/// # Errors
/// Returns an invalid chunk position or source evidence error.
pub fn capture_chunk_origins(
    source: &ModelSource,
    index: usize,
    uses: &SourceUses,
) -> io::Result<ModelChunkOrigins> {
    let [type_id, kind, index, file, usage] = match ChunkTypeEvidence::capture(source, index, uses)?
    {
        Some(evidence) => evidence.origins(source, uses)?,
        None => std::array::from_fn(|_| uses.origin(&[], false)),
    };
    Ok(ModelChunkOrigins {
        type_id,
        kind,
        index,
        file,
        usage,
    })
}

/// Writes chunk identity, file role and reader-use origins at the output path.
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
    let origins = capture_chunk_origins(source, index, uses)?;
    for (name, origin) in [
        ("type_id", origins.type_id),
        ("type", origins.kind),
        ("index", origins.index),
        ("file", origins.file),
        ("usage", origins.usage),
    ] {
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
    file: ModelFile,
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
        Ok(Some(Self {
            scope,
            kind,
            file: chunk.file,
        }))
    }

    fn type_id(&self, uses: &SourceUses) -> FieldOrigin {
        uses.origin(std::slice::from_ref(&self.kind), self.scope.complete)
    }

    fn origins(self, model: &ModelSource, uses: &SourceUses) -> io::Result<[FieldOrigin; 5]> {
        let numeric = self.type_id(uses);
        let signature = self.scope.chunk_header(ChunkHeaderField::Signature)?;
        let table_offset = self.scope.chunk_header(ChunkHeaderField::TableOffset)?;
        let count = self.scope.chunk_header(ChunkHeaderField::RowCount)?;
        let role = self.file_origin(model, uses, [&signature, &table_offset, &count])?;
        let usage = uses.origin(
            &[
                self.scope.selection_at(&self.kind)?,
                self.scope.selection_at(&signature)?,
            ],
            false,
        );
        Ok([
            numeric,
            uses.origin(&[self.kind, signature], self.scope.complete),
            uses.origin(&[table_offset, count], self.scope.complete),
            role,
            usage,
        ])
    }

    fn file_origin(
        &self,
        model: &ModelSource,
        uses: &SourceUses,
        header: [&SourceUse; 3],
    ) -> io::Result<FieldOrigin> {
        let mut contributors = Vec::with_capacity(header.len() + 2);
        contributors.extend(header.into_iter().cloned());
        contributors.push(self.scope.directory_path_selector()?);
        let primary = self.file == ModelFile::Model;
        if !primary && let Some(root) = ModelGeometryScope::capture(uses, model, ModelFile::Model)?
        {
            contributors.push(root.directory_path_selector()?);
        }
        Ok(uses.origin(&contributors, false))
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
