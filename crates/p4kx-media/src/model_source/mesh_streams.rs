// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    MeshChunkSource, MeshStreamHeader, MeshStreamOccurrence, MeshStreamSource,
};

use super::super::mesh_record::{self as wire, CountBasis, MeshRecord, Stream, StreamDisposition};
use super::super::origins;
use super::{MeshProjection, missing, range_of};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[derive(Clone, Copy, Default)]
pub(super) enum IndexStream<'a> {
    #[default]
    Missing,
    Unique(usize, MeshStreamSource<'a>),
    Invalid,
    Ambiguous,
}

impl<'a> IndexStream<'a> {
    fn observe(
        &mut self,
        ordinal: usize,
        header: MeshStreamHeader,
        stream: Option<MeshStreamSource<'a>>,
    ) {
        if !header.is_index_stream() {
            return;
        }
        *self = if matches!(self, Self::Missing) {
            stream
                .filter(|_| matches!(header.stride(), 2 | 4))
                .map_or(Self::Invalid, |stream| Self::Unique(ordinal, stream))
        } else {
            Self::Ambiguous
        };
    }
}

pub(super) fn emit<'input>(
    coordinate: &MeshProjection<'_, '_>,
    mesh: &MeshChunkSource<'input>,
    writer: &mut MediaWriter<'_>,
) -> Result<IndexStream<'input>, MediaError> {
    let mut index = IndexStream::Missing;
    for (ordinal, occurrence) in mesh.streams().enumerate() {
        writer.checkpoint()?;
        match occurrence {
            Ok(MeshStreamOccurrence::Complete(stream)) => {
                let header = stream.header();
                index.observe(ordinal, header, Some(stream));
                write(coordinate, ordinal, header, Some(stream), writer)?;
            }
            Ok(MeshStreamOccurrence::Incomplete(header)) => {
                index.observe(ordinal, header, None);
                write(coordinate, ordinal, header, None, writer)?;
            }
            Err(error) => {
                write_broken_header(coordinate, ordinal, &error, writer)?;
                if !matches!(index, IndexStream::Ambiguous) {
                    index = IndexStream::Invalid;
                }
                break;
            }
            _ => return Err(missing()),
        }
    }
    Ok(index)
}

fn write(
    coordinate: &MeshProjection<'_, '_>,
    ordinal: usize,
    header: MeshStreamHeader,
    stream: Option<MeshStreamSource<'_>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = coordinate.origins.prepared()?;
    let tag = header.tag_range();
    let stride = header.stride_range();
    let (basis, positions) = if header.is_index_stream() {
        (CountBasis::IndexCount, 12..16)
    } else {
        (CountBasis::VertexCount, 8..12)
    };
    let count = coordinate.relative(positions);
    for (field, range) in [
        ("tag", tag.clone()),
        ("stride_bytes", stride.clone()),
        ("count_basis", tag.clone()),
    ] {
        coordinate.bind(&mut prepared, field, range, writer)?;
    }
    let evidence = bind_extent(
        coordinate,
        &mut prepared,
        tag.start..stride.end,
        count,
        writer,
    )?;
    let (disposition, status, issues) = stream_health(
        &mut prepared,
        evidence,
        stream.is_some(),
        header.is_recognized(),
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(
            wire::STREAM,
            vec![coordinate.ordinal as u64, ordinal as u64],
        ),
        status,
        &MeshRecord::MeshStream(Stream {
            tag: Some(header.tag()),
            stride_bytes: Some(header.stride()),
            count: Some(header.count()),
            count_basis: Some(basis),
            payload: stream.map(|stream| range_of(stream.payload_range())),
            padding: stream.map(|stream| range_of(stream.padding_range())),
            retained_tail: stream
                .is_none()
                .then(|| range_of(tag.start..coordinate.chunk.offset() + coordinate.chunk.size())),
            disposition,
        }),
        &issues,
    )
}

fn bind_extent(
    coordinate: &MeshProjection<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    header: std::ops::Range<u64>,
    count: std::ops::Range<u64>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let selection = coordinate.merge(&coordinate.layout, header, "meshStreamHeader", writer)?;
    let evidence = coordinate.merge(&selection, count, "meshStreamCount", writer)?;
    for field in [
        "/subresource/indices",
        "/data/payload/disposition",
        "/data/payload/payload",
        "/data/payload/padding",
        "/data/payload/retained_tail",
        "/data/payload/count",
    ] {
        origins::at(prepared, field, evidence.clone(), writer)?;
    }
    Ok(evidence)
}

fn stream_health(
    prepared: &mut PreparedRecordOrigins,
    evidence: OriginContributions,
    bounded: bool,
    recognized: bool,
    writer: &MediaWriter<'_>,
) -> Result<(StreamDisposition, Status, Vec<MediaIssue>), MediaError> {
    if bounded {
        return Ok((StreamDisposition::Bounded, Status::Complete, Vec::new()));
    }
    if !recognized {
        return Ok((StreamDisposition::OpaqueTail, Status::Complete, Vec::new()));
    }
    origins::at(prepared, "/status", evidence.clone(), writer)?;
    origins::at(prepared, "/issues", evidence, writer)?;
    let issue = MediaIssue::new(
        "model.mesh-stream-extent",
        "stream",
        Severity::Warning,
        "Original stream extent crosses its mesh chunk.",
    );
    Ok((StreamDisposition::Incomplete, Status::Partial, vec![issue]))
}

fn write_broken_header(
    coordinate: &MeshProjection<'_, '_>,
    ordinal: usize,
    error: &p4kx_extraction::model::CorruptChunk,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = coordinate.origins.prepared()?;
    let evidence = coordinate.merge(
        &coordinate.layout,
        coordinate.chunk.offset()..coordinate.chunk.offset() + coordinate.chunk.size(),
        "meshStreamInvalidHeader",
        writer,
    )?;
    for path in [
        "/subresource/indices",
        "/data/payload",
        "/status",
        "/issues",
    ] {
        origins::at(&mut prepared, path, evidence.clone(), writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(
            wire::STREAM,
            vec![coordinate.ordinal as u64, ordinal as u64],
        ),
        Status::Partial,
        &MeshRecord::MeshStream(Stream {
            tag: None,
            stride_bytes: None,
            count: None,
            count_basis: None,
            payload: None,
            padding: None,
            retained_tail: None,
            disposition: StreamDisposition::MalformedHeader,
        }),
        &[MediaIssue::new(
            "model.mesh-stream-header",
            "stream",
            Severity::Warning,
            error.to_string(),
        )],
    )
}
