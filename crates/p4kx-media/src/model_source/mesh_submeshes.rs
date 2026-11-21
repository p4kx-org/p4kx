// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{MeshChunkSource, MeshCounts, MeshStreamSource, MeshSubmeshSource};

use super::super::mesh_record::{self as wire, MeshRecord, Submesh, VertexMapping};
use super::super::origins;
use super::streams::IndexStream;
use super::{MeshProjection, range_of};
use crate::origin_contributions::OriginContributions;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(super) fn emit(
    coordinate: &MeshProjection<'_, '_>,
    mesh: &MeshChunkSource<'_>,
    index: IndexStream<'_>,
    reader: &RequiredReader<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for row in mesh.submeshes() {
        writer.checkpoint()?;
        let mut prepared = coordinate.origins.prepared()?;
        let start = row.range().start;
        for (name, range) in [
            ("material_id", start..start + 2),
            ("node_id", start + 2..start + 4),
            ("first_index", start + 4..start + 8),
            ("index_count", start + 8..start + 12),
            ("vertex_page_base", row.page_base_range()),
        ] {
            coordinate.bind(&mut prepared, name, range, writer)?;
        }
        let origin = coordinate.merge(&coordinate.layout, row.range(), "meshSubmeshRow", writer)?;
        origins::at(
            &mut prepared,
            "/subresource/indices",
            origin.clone(),
            writer,
        )?;
        let (mapping, evidence) = map_index(
            coordinate,
            row,
            mesh.counts(),
            &index,
            origin,
            reader,
            writer,
        )?;
        origins::at(
            &mut prepared,
            "/data/payload/source_vertices",
            evidence,
            writer,
        )?;
        prepared.write(
            writer,
            &Subresource::new(
                wire::SUBMESH,
                vec![coordinate.ordinal as u64, row.ordinal() as u64],
            ),
            Status::Complete,
            &MeshRecord::MeshSubmesh(Submesh {
                material_id: row.material(),
                node_id: row.node(),
                first_index: row.first_index(),
                index_count: row.index_count(),
                vertex_page_base: row.page_base(),
                source_vertices: mapping,
            }),
            &[],
        )?;
    }
    Ok(())
}

fn map_index(
    coordinate: &MeshProjection<'_, '_>,
    row: MeshSubmeshSource,
    counts: MeshCounts,
    index: &IndexStream<'_>,
    origin: OriginContributions,
    reader: &RequiredReader<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(VertexMapping, OriginContributions), MediaError> {
    if row.index_count() == 0 {
        return Ok((VertexMapping::NoIndices, origin));
    }
    let selection = coordinate.merge(
        &origin,
        coordinate.relative(8..16),
        "meshIndexCounts",
        writer,
    )?;
    let IndexStream::Unique(ordinal, stream) = *index else {
        let failed = match index {
            IndexStream::Missing => VertexMapping::MissingIndexStream,
            IndexStream::Invalid => VertexMapping::InvalidIndexStream,
            IndexStream::Ambiguous => VertexMapping::AmbiguousIndexStreams,
            IndexStream::Unique(..) => return Err(super::missing()),
        };
        let evidence = coordinate.merge(
            &selection,
            coordinate.chunk.offset()..coordinate.chunk.offset() + coordinate.chunk.size(),
            "meshIndexStreamInventory",
            writer,
        )?;
        return Ok((failed, evidence));
    };
    let header = stream.header();
    let selection = coordinate.merge(
        &selection,
        header.tag_range().start..header.stride_range().end,
        "meshIndexStreamLayout",
        writer,
    )?;
    let admitted = row.vertices(stream, counts);
    let Ok(indices) = admitted else {
        return Ok((VertexMapping::InvalidIndexRange, selection));
    };
    reader.charge_metadata_work(u64::from(row.index_count()))?;
    verify_indices(
        coordinate, row, stream, ordinal, indices, &selection, writer,
    )
}

fn verify_indices(
    coordinate: &MeshProjection<'_, '_>,
    row: MeshSubmeshSource,
    stream: MeshStreamSource<'_>,
    ordinal: usize,
    indices: impl Iterator<
        Item = Result<
            p4kx_extraction::model::MeshVertexReference,
            p4kx_extraction::model::CorruptChunk,
        >,
    >,
    selection: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(VertexMapping, OriginContributions), MediaError> {
    let base = stream.payload_range().start;
    let width = u64::from(stream.header().stride());
    let start = base + u64::from(row.first_index()) * width;
    let end = start + u64::from(row.index_count()) * width;
    let range = start..end;
    let source = coordinate.merge(selection, range.clone(), "meshOriginalIndexWords", writer)?;
    for (offset, index) in indices.enumerate() {
        if offset.is_multiple_of(1024) {
            writer.checkpoint()?;
        }
        if index.is_err() {
            return Ok((VertexMapping::InvalidSourceVertex, source));
        }
    }
    Ok((
        VertexMapping::StoredIndices {
            stream: ordinal,
            stride_bytes: stream.header().stride(),
            indices: range_of(range),
        },
        source,
    ))
}
