// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::path::Path;

use p4kx_extraction::sound::{WemReadError, WemSource, WemSourceChunk};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use crate::audio_wem_annotation_origins::{AnnotationOrigins, parent_route, parent_tag};
use crate::audio_wem_annotation_payload::{AnnotationFailure, AnnotationPayload, AnnotationPhase};
use crate::audio_wem_diagnostic;
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, audio_wem_cue, audio_wem_list, error,
};

pub(crate) const FAILURE_NAMESPACE: &str = "audio-wem-annotation-failure";

pub(crate) fn directory<R: Read + Seek>(
    projection: &WemProjection<'_>,
    source: &mut WemSource<R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut cursor = source.chunk_cursor();
    while let Some(result) = source.next_chunk(&mut cursor) {
        let entry = match result {
            Ok(entry) => entry,
            Err(cause) => {
                return projection.read_failure(
                    crate::audio_payload::WemPhase::ChunkDirectory,
                    cause,
                    writer,
                );
            }
        };
        let chunk = entry.chunk();
        projection.chunk(chunk, writer)?;
        match &chunk.id {
            b"cue " => audio_wem_cue::project(projection, source, &entry, writer)?,
            b"LIST" => audio_wem_list::project(projection, source, &entry, writer)?,
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn input_error(projection: &WemProjection<'_>, cause: WemReadError) -> MediaError {
    match cause {
        WemReadError::Io(cause) => error::io(Path::new(projection.path), cause),
        other => MediaError::Contract(format!("WEM annotation input proof failed: {other}")),
    }
}

pub(crate) fn failure(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    phase: AnnotationPhase,
    cause: WemReadError,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let WemReadError::Format(cause) = cause else {
        return Err(input_error(projection, cause));
    };
    let name = match phase {
        AnnotationPhase::Cue => "cue",
        AnnotationPhase::List => "LIST",
    };
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let indices = [parent.ordinal];
    let body = [parent.body_offset];
    let route = parent_route(&indices);
    origins.bind_occurrence(route, None)?;
    let phase_origin = origins.capture([route, parent_tag(&body)])?;
    origins.field("/data/payload/phase", phase_origin)?;
    let mut capture = writer.capture_origins();
    capture.include(
        &OriginContributions::unknown(&projection.input.owner),
        writer,
    )?;
    let observed = projection.capture(
        [(
            OriginSite::new("wemAnnotationFailure", &body, name),
            SourceUseRole::Conversion,
        )],
        writer,
    )?;
    capture.include(&observed, writer)?;
    let evidence = capture.finish();
    origins.subtree("/data/payload/reason", evidence.clone())?;
    origins.subtree("/issues", evidence)?;
    origins.finish().write(
        writer,
        &projection.subresource(FAILURE_NAMESPACE, &[parent.ordinal])?,
        Status::Failed,
        &AnnotationPayload::Failure(AnnotationFailure {
            chunk_ordinal: DecimalU64::new(parent.ordinal),
            phase,
            reason: audio_wem_diagnostic::reason(&cause),
        }),
        &[MediaIssue::new(
            "audio.wem.invalid-annotation",
            name,
            Severity::Error,
            cause.to_string(),
        )],
    )
}
