// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{Chunk, ChunkDirectory, ivo};
use p4kx_extraction::model::MeshChunkSource;

use super::mesh_record::{self as wire, Mesh, MeshRecord};
use super::origins::{self, ModelOrigins};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{
    MediaError, MediaIssue, MediaWriter, OffsetBasis, Severity, SourceRange, Status, Subresource,
};

#[path = "mesh_streams.rs"]
mod streams;
#[path = "mesh_submeshes.rs"]
mod submeshes;

pub(super) fn emit(
    file: &ChunkDirectory,
    ordinal: usize,
    reader: &mut RequiredReader<'_>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let chunk = file.chunks().get(ordinal).ok_or_else(missing)?;
    if !matches!(chunk.chunk_type(), ivo::SKIN | ivo::SKIN_2) {
        return Ok(());
    }
    let row = file.table_row(ordinal).ok_or_else(missing)?;
    let mut origin = writer.capture_origins();
    let extent = origins::chunk_extent(file.table_range(), row.offset, row.size, origins, writer)?;
    origin.include_original(&extent.end)?;
    for (name, span) in [("meshType", row.kind), ("meshVersion", row.version)] {
        origin.include_original(&origins.payload(
            span,
            name,
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    let coordinate = MeshProjection {
        chunk,
        ordinal,
        layout: origin.finish(),
        origins,
    };
    if !MeshChunkSource::supports(chunk) {
        return coordinate.write_head(Mesh::UnsupportedVersion, None, writer);
    }
    let bytes = reader.read_chunk(chunk.offset()..chunk.offset() + chunk.size())?;
    match MeshChunkSource::read(chunk, &bytes) {
        Ok(Some(mesh)) => coordinate.project(&mesh, reader, writer),
        Ok(None) => Err(missing()),
        Err(cause) => coordinate.write_head(Mesh::Malformed, Some(cause.to_string()), writer),
    }
}

struct MeshProjection<'a, 'input> {
    chunk: &'a Chunk,
    ordinal: usize,
    layout: OriginContributions,
    origins: &'a ModelOrigins<'input>,
}

impl MeshProjection<'_, '_> {
    fn project(
        &self,
        mesh: &MeshChunkSource<'_>,
        reader: &RequiredReader<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let counts = mesh.counts();
        let [model_bounds_bits, scaling_bounds_bits] = mesh.bounds_bits();
        self.write_head(
            Mesh::Measured {
                vertices: counts.vertices,
                indices: counts.indices,
                submeshes: counts.submeshes,
                extra_words: counts.extra_words,
                model_bounds_bits,
                scaling_bounds_bits,
            },
            None,
            writer,
        )?;
        let index = streams::emit(self, mesh, writer)?;
        submeshes::emit(self, mesh, index, reader, writer)
    }

    fn write_head(
        &self,
        mesh: Mesh,
        issue: Option<String>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        origins::at(
            &mut prepared,
            "/subresource/indices",
            self.layout.clone(),
            writer,
        )?;
        let mut health = self.layout.clone();
        if matches!(mesh, Mesh::Measured { .. }) {
            for (name, span) in [
                ("vertices", 8..12),
                ("indices", 12..16),
                ("submeshes", 16..20),
                ("extra_words", 76..80),
                ("model_bounds_bits", 24..48),
                ("scaling_bounds_bits", 48..72),
            ] {
                self.bind(&mut prepared, name, self.relative(span), writer)?;
            }
        }
        let issues = if let Some(detail) = issue {
            health = self.merge(
                &health,
                self.chunk.offset()..self.chunk.offset() + self.chunk.size(),
                "meshInvalidLayout",
                writer,
            )?;
            let fault = MediaIssue::new("model.mesh-layout", "chunk", Severity::Warning, detail);
            origins::at(&mut prepared, "/issues", health.clone(), writer)?;
            origins::at(&mut prepared, "/status", health.clone(), writer)?;
            vec![fault]
        } else {
            Vec::new()
        };
        origins::at(&mut prepared, "/data/payload/disposition", health, writer)?;
        let status = if issues.is_empty() {
            Status::Complete
        } else {
            Status::Partial
        };
        prepared.write(
            writer,
            &Subresource::new(wire::MESH, vec![self.ordinal as u64]),
            status,
            &MeshRecord::Mesh(mesh),
            &issues,
        )
    }

    fn bind(
        &self,
        prepared: &mut PreparedRecordOrigins,
        name: &str,
        range: Range<u64>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let value = self
            .origins
            .payload(range, name, SourceUseRole::Payload, writer)?;
        origins::at(prepared, &format!("/data/payload/{name}"), value, writer)
    }

    fn merge(
        &self,
        a: &OriginContributions,
        range: Range<u64>,
        name: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut facts = writer.capture_origins();
        facts.include_original(a)?;
        facts.include_original(&self.origins.payload(
            range,
            name,
            SourceUseRole::Selector,
            writer,
        )?)?;
        Ok(facts.finish())
    }

    const fn relative(&self, range: Range<u64>) -> Range<u64> {
        self.chunk.offset() + range.start..self.chunk.offset() + range.end
    }
}

const fn range_of(span: Range<u64>) -> SourceRange {
    SourceRange::new(OffsetBasis::Entry, span.start, span.end)
}

fn missing() -> MediaError {
    MediaError::Contract("IVO mesh lost its original chunk table coordinate".into())
}
