// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::path::Path;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::sound::{
    WemChunkEntry, WemList, WemListItem, WemListPayload, WemSource, WemSourceChunk, WemTextUtf8,
};

use crate::audio_wem_annotation;
use crate::audio_wem_annotation_payload::{
    AnnotationPayload, AnnotationPhase, ListHeader, ListItem,
};
use crate::audio_wem_projection::WemProjection;
use crate::{
    MediaError, MediaWriter, Status, audio_wem_list_origins, audio_wem_list_payload, error,
};

pub(crate) const LIST_NAMESPACE: &str = "audio-wem-list";
pub(crate) const ITEM_NAMESPACE: &str = "audio-wem-list-item";

pub(crate) fn project<R: Read + Seek>(
    projection: &WemProjection<'_>,
    source: &mut WemSource<R>,
    entry: &WemChunkEntry,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let parent = entry.chunk();
    let mut list = match source.list(entry) {
        Ok(list) => list,
        Err(cause) => {
            return audio_wem_annotation::failure(
                projection,
                &parent,
                AnnotationPhase::List,
                cause,
                writer,
            );
        }
    };
    let list_type = list.list_type();
    header(projection, &parent, list_type, writer)?;
    while let Some(result) = list.next() {
        let item = match result {
            Ok(item) => item,
            Err(cause) => {
                return audio_wem_annotation::failure(
                    projection,
                    &parent,
                    AnnotationPhase::List,
                    cause,
                    writer,
                );
            }
        };
        let text = read_utf8(projection, &mut list, &item, writer)?;
        emit_item(
            projection,
            &parent,
            list_type,
            &item,
            text.as_deref(),
            writer,
        )?;
    }
    Ok(())
}

fn header(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    list_type: [u8; 4],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_wem_list_origins::header(projection, parent, writer)?.write(
        writer,
        &projection.subresource(LIST_NAMESPACE, &[parent.ordinal])?,
        Status::Complete,
        &AnnotationPayload::List(ListHeader {
            chunk_ordinal: DecimalU64::new(parent.ordinal),
            list_type,
        }),
        &[],
    )
}

fn emit_item(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    list_type: [u8; 4],
    item: &WemListItem,
    text: Option<&str>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let chunk = item.chunk();
    let end = chunk.body_end_exclusive();
    let projected = audio_wem_list_payload::content(projection, item.payload(), text)?;
    let payload = ListItem {
        chunk_ordinal: DecimalU64::new(parent.ordinal),
        ordinal: DecimalU64::new(chunk.ordinal),
        tag: chunk.id,
        header_range: projection.range(chunk.header_offset, chunk.body_offset)?,
        body_range: projection.range(chunk.body_offset, end)?,
        padding_range: (end < chunk.next_offset)
            .then(|| projection.range(end, chunk.next_offset))
            .transpose()?,
        content: projected.content,
    };
    audio_wem_list_origins::item(projection, parent, list_type, item, writer)?.write(
        writer,
        &projection.subresource(ITEM_NAMESPACE, &[parent.ordinal, chunk.ordinal])?,
        projected.status,
        &AnnotationPayload::Item(&payload),
        projected.issue.as_slice(),
    )
}

fn read_utf8<R: Read + Seek>(
    projection: &WemProjection<'_>,
    list: &mut WemList<'_, R>,
    item: &WemListItem,
    writer: &MediaWriter<'_>,
) -> Result<Option<String>, MediaError> {
    let (WemListPayload::Label(text) | WemListPayload::Note(text)) = item.payload() else {
        return Ok(None);
    };
    match text.utf8 {
        WemTextUtf8::Ascii | WemTextUtf8::Valid => {}
        WemTextUtf8::Invalid => return Ok(None),
        _ => {
            return Err(MediaError::Contract(
                "unrecognized WEM UTF-8 validation".into(),
            ));
        }
    }
    let length = text.text_range.end - text.text_range.start;
    let limit = writer.limits().record_bytes;
    if length > limit {
        return Err(MediaError::Limit {
            budget: "record_bytes",
            limit,
        });
    }
    let length = usize::try_from(length).map_err(|cause| buffer_error(projection, cause))?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(length)
        .map_err(|cause| buffer_error(projection, cause))?;
    buffer.resize(length, 0);
    list.read_text(item, &mut buffer)
        .map_err(|cause| audio_wem_annotation::input_error(projection, cause))?;
    String::from_utf8(buffer).map(Some).map_err(|cause| {
        error::io(
            Path::new(projection.path),
            io::Error::new(io::ErrorKind::InvalidData, cause.utf8_error()),
        )
    })
}

fn buffer_error(
    projection: &WemProjection<'_>,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> MediaError {
    error::io(
        Path::new(projection.path),
        io::Error::new(io::ErrorKind::OutOfMemory, cause),
    )
}
