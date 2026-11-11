// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::sound::{WemCuePoint, WemSourceChunk};

use crate::audio_wem_annotation_origins::{
    AnnotationOrigins, WORD_BYTES, body_length, byte_order, parent_route, parent_tag,
};
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn header(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let indices = [parent.ordinal];
    let body = [parent.body_offset];
    let route = parent_route(&indices);
    origins.bind_occurrence(route, None)?;
    origins.word(
        "/data/payload/declared_points",
        "cueCount",
        parent.body_offset,
        WORD_BYTES,
        &[route, parent_tag(&body), byte_order()],
    )?;
    Ok(origins.finish())
}

pub(crate) fn point(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    point: &WemCuePoint,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let indices = [parent.ordinal];
    let ordinal = [parent.ordinal, point.ordinal];
    let body = [parent.body_offset];
    let route = parent_route(&indices);
    let child = (
        OriginSite::new("wemCuePrefix", &ordinal, "pointRouting"),
        SourceUseRole::Selector,
    );
    let position = origins.bind_occurrence(route, Some(child))?;
    origins.range("/data/payload/point_range", &position, &position)?;
    let numeric = [route, parent_tag(&body), child, byte_order()];
    for (path, name, index) in [
        ("/data/payload/id", "cueId", 0),
        ("/data/payload/position", "playPosition", 1),
        ("/data/payload/chunk_start", "chunkStart", 3),
        ("/data/payload/block_start", "blockStart", 4),
        ("/data/payload/sample_offset", "sampleOffset", 5),
    ] {
        origins.word(
            path,
            name,
            point.offset + index * WORD_BYTES,
            WORD_BYTES,
            &numeric,
        )?;
    }
    origins.fourcc(
        "/data/payload/target_chunk",
        "targetChunk",
        point.offset + 2 * WORD_BYTES,
        &[route, parent_tag(&body), child],
    )?;
    Ok(origins.finish())
}

pub(crate) fn tail(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    count: u32,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let indices = [parent.ordinal];
    let body = [parent.body_offset];
    let count_span = [parent.body_offset, parent.body_offset + WORD_BYTES];
    let prefix = [parent.ordinal, u64::from(count)];
    let route = parent_route(&indices);
    origins.bind_occurrence(route, None)?;
    let basis = [
        route,
        parent_tag(&body),
        byte_order(),
        (
            OriginSite::new("wemAnnotationField", &count_span, "cueCount"),
            SourceUseRole::Conversion,
        ),
        (
            OriginSite::new("wemCuePrefix", &prefix, "completePointRouting"),
            SourceUseRole::Selector,
        ),
    ];
    let start = origins.capture(basis)?;
    let end = origins.capture(basis.into_iter().chain([body_length(&body)]))?;
    origins.range("/data/payload/trailing_range", &start, &end)?;
    Ok(origins.finish())
}
