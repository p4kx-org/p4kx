// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveByteRange, ArchiveMetadataVersion, DecimalU64, MetadataAccess, MetadataRange,
    MetadataSpan,
};

use crate::archive_raw::{ArchiveRawChunk, ArchiveRawKey, ArchiveRawSpan};
use crate::archive_section::{ArchiveSection, SECTIONS};
use crate::metadata_input::MetadataInput;
use crate::{ByteOrder, ChunkLayout, DataType, MediaError, MediaWriter, Status, Subresource};

pub(crate) const RAW_NAMESPACE: &str = "archive-raw-chunk";

struct Retained {
    range: MetadataRange,
    series: ArchiveRawKey,
}

pub(crate) struct ArchiveRetention {
    sections: Vec<Retained>,
}

impl ArchiveRetention {
    pub(crate) fn new() -> Self {
        Self {
            sections: Vec::with_capacity(SECTIONS.len()),
        }
    }

    pub(crate) fn section<'source>(
        &mut self,
        section: ArchiveSection,
        span: MetadataSpan<'source>,
        access: &mut MetadataAccess<'source, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ArchiveRawSpan, MediaError> {
        if let Some(raw) = self.find(span.range()) {
            return Ok(raw);
        }
        let series = ArchiveRawKey::Section { section };
        stream(series, span, access, writer)?;
        if self.sections.len() >= SECTIONS.len() {
            return Err(MediaError::Contract(
                "metadata section inventory exceeded its fixed dimension".into(),
            ));
        }
        self.sections.push(Retained {
            range: span.range(),
            series,
        });
        Ok(reference(series, span.range()))
    }

    pub(crate) fn record<'source>(
        &self,
        ordinal: u64,
        span: MetadataSpan<'source>,
        access: &mut MetadataAccess<'source, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ArchiveRawSpan, MediaError> {
        if let Some(raw) = self.find(span.range()) {
            return Ok(raw);
        }
        let series = ArchiveRawKey::RecoveredRecord {
            record: DecimalU64::new(ordinal),
        };
        stream(series, span, access, writer)?;
        Ok(reference(series, span.range()))
    }

    fn find(&self, range: MetadataRange) -> Option<ArchiveRawSpan> {
        self.sections
            .iter()
            .find(|item| contains(item.range, range))
            .map(|item| reference(item.series, range))
    }

    pub(crate) fn existing(&self, span: MetadataSpan<'_>) -> Result<ArchiveRawSpan, MediaError> {
        self.find(span.range()).ok_or_else(|| {
            MediaError::Contract("metadata table row has no fully retained owning section".into())
        })
    }
}

pub(crate) fn within(
    parent: &ArchiveRawSpan,
    span: MetadataSpan<'_>,
) -> Result<ArchiveRawSpan, MediaError> {
    let range = span.range();
    let parent_end = parent
        .range
        .offset
        .get()
        .checked_add(parent.range.length.get());
    let end = range.offset().checked_add(range.len());
    if range.offset() < parent.range.offset.get()
        || parent_end
            .zip(end)
            .is_none_or(|(parent, child)| child > parent)
    {
        return Err(MediaError::Contract(
            "metadata subrange is outside its retained parent".into(),
        ));
    }
    Ok(reference(parent.series, range))
}

const fn contains(outer: MetadataRange, inner: MetadataRange) -> bool {
    inner.offset() >= outer.offset()
        && inner.len() <= outer.len()
        && inner.offset() - outer.offset() <= outer.len() - inner.len()
}

const fn reference(series: ArchiveRawKey, range: MetadataRange) -> ArchiveRawSpan {
    ArchiveRawSpan {
        series,
        range: ArchiveByteRange::of(range),
    }
}

fn stream<'source>(
    series: ArchiveRawKey,
    span: MetadataSpan<'source>,
    access: &mut MetadataAccess<'source, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    const MAX_EXACT_JSON_INTEGER: u64 = (1 << 53) - 1;
    let capacity = writer.limits().chunk_bytes.min(MAX_EXACT_JSON_INTEGER);
    let mut offset = 0;
    let mut ordinal = 0;
    loop {
        writer.checkpoint()?;
        let count = (span.range().len() - offset).min(capacity);
        let part = span.subspan(offset, count)?;
        let layout = ChunkLayout::new(
            DataType::U8,
            ByteOrder::Little,
            count,
            "opaque bytes; exact owning-source extent is in archive-raw-chunk.range",
        );
        let mut input = MetadataInput::new(access, part);
        let chunk = writer.metadata_chunk(layout, &mut input)?;
        let data = ArchiveRawChunk {
            version: ArchiveMetadataVersion::V1,
            series,
            ordinal: DecimalU64::new(ordinal),
            range: ArchiveByteRange::of(part.range()),
            chunk,
        };
        writer.archive_layout_record(
            &Subresource::new(RAW_NAMESPACE, indices(series, ordinal)?),
            Status::Complete,
            &data,
            &[],
        )?;
        offset += count;
        if offset == span.range().len() {
            return Ok(());
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| MediaError::Contract("metadata chunk ordinal overflow".into()))?;
    }
}

fn indices(series: ArchiveRawKey, ordinal: u64) -> Result<Vec<u64>, MediaError> {
    match series {
        ArchiveRawKey::Section { section } => {
            let index = SECTIONS
                .iter()
                .position(|(candidate, _)| *candidate == section)
                .ok_or_else(|| MediaError::Contract("unknown retained metadata section".into()))?;
            Ok(vec![
                0,
                u64::try_from(index).map_err(|_| {
                    MediaError::Contract("section index exceeds source width".into())
                })?,
                ordinal,
            ])
        }
        ArchiveRawKey::RecoveredRecord { record } => Ok(vec![1, record.get(), ordinal]),
    }
}
