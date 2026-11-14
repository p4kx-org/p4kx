// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::sound::WemSourceChunk;

use crate::audio_record_origins as origins;
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn chunk(
    projection: &WemProjection<'_>,
    chunk: &WemSourceChunk,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = origins::prepare(projection.input)?;
    let ordinal = [chunk.ordinal];
    let routing = (
        OriginSite::new("wemRiffDirectory", &ordinal, "chunkRouting"),
        SourceUseRole::Selector,
    );
    let order = (
        OriginSite::new("wemRiffHeader", &[], "signature"),
        SourceUseRole::Conversion,
    );
    let body = [chunk.body_offset];
    let length = (
        OriginSite::new("wemChunk", &body, "bodyLength"),
        SourceUseRole::Conversion,
    );
    let directory = projection.capture([routing], writer)?;
    let extent = projection.capture([routing, order, length], writer)?;
    origins::field(
        &mut prepared,
        "/data/payload/ordinal",
        directory.clone(),
        writer,
    )?;
    projection.indices(&mut prepared, directory.clone(), writer)?;
    tag(projection, chunk, routing, &mut prepared, writer)?;
    origins::range(
        &mut prepared,
        "/data/payload/header_range",
        &directory,
        &directory,
        writer,
    )?;
    origins::range(
        &mut prepared,
        "/data/payload/body_range",
        &directory,
        &extent,
        writer,
    )?;
    let next = [chunk.body_end_exclusive(), chunk.next_offset];
    let padding = projection.capture(
        [
            routing,
            order,
            length,
            (
                OriginSite::new("wemRiffDirectory", &next, "paddingDecision"),
                SourceUseRole::Selector,
            ),
        ],
        writer,
    )?;
    let path = "/data/payload/padding_range";
    if chunk.body_end_exclusive() < chunk.next_offset {
        origins::range(&mut prepared, path, &extent, &padding, writer)?;
    } else {
        origins::field(&mut prepared, path, padding, writer)?;
    }
    Ok(prepared)
}

fn tag(
    projection: &WemProjection<'_>,
    chunk: &WemSourceChunk,
    routing: (OriginSite<'_>, SourceUseRole),
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let body = [chunk.body_offset];
    let tag = projection.capture(
        [
            routing,
            (
                OriginSite::new("wemChunk", &body, "id"),
                SourceUseRole::Payload,
            ),
        ],
        writer,
    )?;
    origins::field(prepared, "/data/payload/tag", tag, writer)?;
    for offset in 0..u64::try_from(chunk.id.len()).map_err(|_| crate::origin_error::counter())? {
        let indices = [chunk.header_offset + offset];
        let evidence = projection.capture(
            [
                routing,
                (
                    OriginSite::new("wemRiffByte", &indices, "chunkIdByte"),
                    SourceUseRole::Payload,
                ),
            ],
            writer,
        )?;
        origins::field(
            prepared,
            &format!("/data/payload/tag/{offset}"),
            evidence,
            writer,
        )?;
    }
    Ok(())
}
