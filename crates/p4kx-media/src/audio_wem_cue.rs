// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::sound::{WemChunkEntry, WemCuePoint, WemSource, WemSourceChunk};

use crate::audio_wem_annotation;
use crate::audio_wem_annotation_origins::WORD_BYTES;
use crate::audio_wem_annotation_payload::{
    AnnotationPayload, AnnotationPhase, CueHeader, CuePoint, CueTail,
};
use crate::audio_wem_projection::WemProjection;
use crate::{MediaError, MediaWriter, Status, audio_wem_cue_origins};

pub(crate) const CUE_NAMESPACE: &str = "audio-wem-cue";
pub(crate) const POINT_NAMESPACE: &str = "audio-wem-cue-point";
pub(crate) const TAIL_NAMESPACE: &str = "audio-wem-cue-tail";
pub(crate) const CUE_POINT_BYTES: u64 = 6 * WORD_BYTES;

pub(crate) fn project<R: Read + Seek>(
    projection: &WemProjection<'_>,
    source: &mut WemSource<R>,
    entry: &WemChunkEntry,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let parent = entry.chunk();
    let mut cue = match source.cue(entry) {
        Ok(cue) => cue,
        Err(cause) => {
            return audio_wem_annotation::failure(
                projection,
                &parent,
                AnnotationPhase::Cue,
                cause,
                writer,
            );
        }
    };
    let count = cue.point_count();
    header(projection, &parent, count, writer)?;
    for result in cue.by_ref() {
        match result {
            Ok(point) => emit_point(projection, &parent, &point, writer)?,
            Err(cause) => {
                return audio_wem_annotation::failure(
                    projection,
                    &parent,
                    AnnotationPhase::Cue,
                    cause,
                    writer,
                );
            }
        }
    }
    let trailing = cue.trailing_range().ok_or_else(|| {
        MediaError::Contract("completed WEM cue cursor has no residual range".into())
    })?;
    tail(projection, &parent, count, &trailing, writer)
}

fn header(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    count: u32,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_wem_cue_origins::header(projection, parent, writer)?.write(
        writer,
        &projection.subresource(CUE_NAMESPACE, &[parent.ordinal])?,
        Status::Complete,
        &AnnotationPayload::Cue(CueHeader {
            chunk_ordinal: DecimalU64::new(parent.ordinal),
            declared_points: count,
        }),
        &[],
    )
}

fn emit_point(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    point: &WemCuePoint,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let payload = AnnotationPayload::Point(CuePoint {
        chunk_ordinal: DecimalU64::new(parent.ordinal),
        ordinal: DecimalU64::new(point.ordinal),
        point_range: projection.range(point.offset, point.offset + CUE_POINT_BYTES)?,
        id: point.id,
        position: point.position,
        target_chunk: point.target_chunk,
        chunk_start: point.chunk_start,
        block_start: point.block_start,
        sample_offset: point.sample_offset,
    });
    audio_wem_cue_origins::point(projection, parent, point, writer)?.write(
        writer,
        &projection.subresource(POINT_NAMESPACE, &[parent.ordinal, point.ordinal])?,
        Status::Complete,
        &payload,
        &[],
    )
}

fn tail(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    count: u32,
    trailing: &Range<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_wem_cue_origins::tail(projection, parent, count, writer)?.write(
        writer,
        &projection.subresource(TAIL_NAMESPACE, &[parent.ordinal])?,
        Status::Complete,
        &AnnotationPayload::CueTail(CueTail {
            chunk_ordinal: DecimalU64::new(parent.ordinal),
            trailing_range: projection.range(trailing.start, trailing.end)?,
        }),
        &[],
    )
}
