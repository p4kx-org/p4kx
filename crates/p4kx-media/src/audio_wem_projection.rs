// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::sound::{WemMetadata, WemReadError, WemSource, WemSourceChunk};
use p4kx_extraction::{
    AudioError, ByteOrder as WemByteOrder, DecimalU64, InputScope, SourceUseRole,
};

use crate::audio_bank_media_origins::EmbeddedWemOrigins;
use crate::audio_payload::{
    AudioPayload, WemChunk, WemDuration, WemHeader, WemLoop, WemOpaqueClass, WemPhase,
};
use crate::audio_record_origins as origins;
use crate::audio_wem_annotation;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    ByteOrder, MediaError, MediaWriter, OffsetBasis, SourceRange, Status, Subresource,
    audio_chunk_origins, audio_wem_origins,
};

pub(crate) const WEM_NAMESPACE: &str = "audio-wem";
pub(crate) const CHUNK_NAMESPACE: &str = "audio-wem-chunk";
pub(crate) const EMBEDDED_INDEX_COUNT: u8 = 2;

struct EmbeddedWem {
    indices: [u64; EMBEDDED_INDEX_COUNT as usize],
    start: u64,
    origins: EmbeddedWemOrigins,
}

pub(crate) struct WemProjection<'source> {
    pub(super) input: &'source MediaOriginInput,
    pub(super) path: &'source str,
    pub(super) byte_length: u64,
    embedded: Option<EmbeddedWem>,
}

pub(crate) fn embedded_namespace(standalone: &str) -> Result<String, MediaError> {
    let suffix = standalone
        .strip_prefix(WEM_NAMESPACE)
        .ok_or_else(|| MediaError::Contract("invalid WEM record namespace".into()))?;
    Ok(format!("audio-bank-wem{suffix}"))
}

pub(crate) fn project<R: Read + Seek>(
    projection: &WemProjection<'_>,
    reader: R,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut source = match WemSource::open(reader, projection.byte_length) {
        Ok(source) => source,
        Err(WemReadError::Format(AudioError::PluginMedia)) => {
            return projection.opaque(WemOpaqueClass::PluginMedia, writer);
        }
        Err(WemReadError::Format(AudioError::MidiMedia)) => {
            return projection.opaque(WemOpaqueClass::MidiMedia, writer);
        }
        Err(cause) => return projection.read_failure(WemPhase::RiffHeader, cause, writer),
    };
    let riff_end = source.riff_end();
    match source.read_metadata() {
        Ok(metadata) => projection.header(&metadata, riff_end, writer)?,
        Err(cause) => projection.read_failure(WemPhase::Summary, cause, writer)?,
    }
    audio_wem_annotation::directory(projection, &mut source, writer)
}

impl<'source> WemProjection<'source> {
    pub(crate) const fn new(
        input: &'source MediaOriginInput,
        path: &'source str,
        byte_length: u64,
    ) -> Self {
        Self {
            input,
            path,
            byte_length,
            embedded: None,
        }
    }

    pub(crate) fn embedded(
        input: &'source MediaOriginInput,
        path: &'source str,
        indices: [u64; EMBEDDED_INDEX_COUNT as usize],
        range: &Range<u64>,
        origins: EmbeddedWemOrigins,
    ) -> Result<Self, MediaError> {
        let byte_length = range
            .end
            .checked_sub(range.start)
            .ok_or_else(geometry_error)?;
        Ok(Self {
            input,
            path,
            byte_length,
            embedded: Some(EmbeddedWem {
                indices,
                start: range.start,
                origins,
            }),
        })
    }

    pub(crate) const fn is_embedded(&self) -> bool {
        self.embedded.is_some()
    }

    pub(crate) const fn index_offset(&self) -> usize {
        if self.is_embedded() {
            EMBEDDED_INDEX_COUNT as usize
        } else {
            0
        }
    }

    pub(crate) fn subresource(
        &self,
        namespace: &'static str,
        local: &[u64],
    ) -> Result<Subresource, MediaError> {
        let Some(embedded) = &self.embedded else {
            return Ok(Subresource::new(namespace, local.to_vec()));
        };
        let indices = embedded.indices.iter().chain(local).copied().collect();
        Ok(Subresource::new(embedded_namespace(namespace)?, indices))
    }

    pub(crate) fn offset(&self, local: u64) -> Result<u64, MediaError> {
        if local > self.byte_length {
            return Err(geometry_error());
        }
        let start = self.embedded.as_ref().map_or(0, |embedded| embedded.start);
        start.checked_add(local).ok_or_else(geometry_error)
    }

    pub(crate) fn range(&self, start: u64, end: u64) -> Result<SourceRange, MediaError> {
        if start > end {
            return Err(geometry_error());
        }
        Ok(SourceRange::new(
            OffsetBasis::Entry,
            self.offset(start)?,
            self.offset(end)?,
        ))
    }

    pub(crate) fn capture<'site>(
        &self,
        sites: impl IntoIterator<Item = (OriginSite<'site>, SourceUseRole)>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let Some(embedded) = &self.embedded else {
            return origins::capture(self.input, sites, writer);
        };
        let mut capture = writer.capture_origins();
        capture.include(&embedded.origins.selection, writer)?;
        for (site, role) in sites {
            capture.add(
                self.input,
                InputScope::Payload,
                site.prefixed(&embedded.indices),
                role,
                writer,
            )?;
        }
        Ok(capture.finish())
    }

    pub(crate) fn indices(
        &self,
        prepared: &mut PreparedRecordOrigins,
        local: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(embedded) = &self.embedded else {
            return origins::indices(prepared, local, writer);
        };
        let mut capture = writer.capture_origins();
        capture.include(&embedded.origins.triplet, writer)?;
        capture.include(&local, writer)?;
        origins::indices(prepared, capture.finish(), writer)?;
        origins::field(
            prepared,
            "/subresource/indices/0",
            embedded.origins.section.clone(),
            writer,
        )?;
        origins::field(
            prepared,
            "/subresource/indices/1",
            embedded.origins.triplet.clone(),
            writer,
        )
    }

    pub(crate) fn header(
        &self,
        metadata: &WemMetadata,
        riff_end: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let trailing = self.byte_length.checked_sub(riff_end).ok_or_else(|| {
            MediaError::Contract("WEM RIFF extent exceeds its retained source".into())
        })?;
        let payload = AudioPayload::Wem(WemHeader {
            byte_order: match metadata.byte_order() {
                WemByteOrder::Little => ByteOrder::Little,
                WemByteOrder::Big => ByteOrder::Big,
            },
            format_tag: metadata.format_tag(),
            codec: metadata.codec().id(),
            channels: metadata.channels(),
            block_align: metadata.block_align(),
            bits_per_sample: metadata.bits_per_sample(),
            channel_layout: metadata.channel_layout(),
            channel_mask: metadata.channel_mask(),
            declared_sample_rate: metadata.declared_sample_rate(),
            sample_rate: metadata.sample_rate(),
            sample_count: metadata.sample_count().map(DecimalU64::new),
            duration: metadata.duration().map(|value| WemDuration {
                seconds: DecimalU64::new(value.as_secs()),
                nanoseconds: value.subsec_nanos(),
            }),
            loop_points: metadata.loop_points().map(|points| WemLoop {
                start: DecimalU64::new(points.start),
                end: DecimalU64::new(points.end),
                frames: DecimalU64::new(points.frames()),
            }),
            riff_end_exclusive: self.offset(riff_end)?,
            trailing_length: DecimalU64::new(trailing),
        });
        audio_wem_origins::header(self, metadata, writer)?.write(
            writer,
            &self.subresource(WEM_NAMESPACE, &[])?,
            Status::Complete,
            &payload,
            &[],
        )
    }

    pub(crate) fn chunk(
        &self,
        chunk: WemSourceChunk,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let body_end = chunk.body_end_exclusive();
        let payload = AudioPayload::WemChunk(WemChunk {
            ordinal: DecimalU64::new(chunk.ordinal),
            tag: chunk.id,
            header_range: self.range(chunk.header_offset, chunk.body_offset)?,
            body_range: self.range(chunk.body_offset, body_end)?,
            padding_range: (body_end < chunk.next_offset)
                .then(|| self.range(body_end, chunk.next_offset))
                .transpose()?,
        });
        audio_chunk_origins::chunk(self, &chunk, writer)?.write(
            writer,
            &self.subresource(CHUNK_NAMESPACE, &[chunk.ordinal])?,
            Status::Complete,
            &payload,
            &[],
        )
    }
}

fn geometry_error() -> MediaError {
    MediaError::Contract("WEM coordinates exceed the retained source range".into())
}
