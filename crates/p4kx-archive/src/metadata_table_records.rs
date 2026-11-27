// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::metadata_access::MetadataAccess;
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_region::MetadataRegion;
use crate::metadata_section::MetadataSection;
use crate::metadata_span::MetadataSpan;
use crate::metadata_table_row::{
    MetadataFreeValue, MetadataInstallValue, MetadataProgressBound, MetadataTableRow,
    MetadataTableValue,
};
use crate::metadata_tables::{FREE_RECORD_BYTES, V2_PROGRESS_BYTES, ZIP_PROGRESS_BYTES};
use crate::metadata_view::MetadataView;

/// Fixed installation or free records sharing the caller's budget and owning-source view.
pub struct MetadataTableRecords<'source> {
    view: MetadataView<'source>,
    section: MetadataSpan<'source>,
    free: bool,
    width: usize,
    ordinal: u64,
    count: u64,
    admitted: usize,
    failed: bool,
}

impl<'source> MetadataTableRecords<'source> {
    pub(crate) fn new(
        view: MetadataView<'source>,
        section: MetadataSection,
    ) -> Result<Option<Self>, MetadataReadError> {
        let Some(span) = view.span(section)? else {
            return Ok(None);
        };
        let free = section == MetadataSection::FreeRanges;
        let width = if free {
            FREE_RECORD_BYTES
        } else if view.metadata().v2_directory().is_some() {
            V2_PROGRESS_BYTES
        } else {
            ZIP_PROGRESS_BYTES
        };
        let width_u64 = u64::try_from(width).map_err(|_| MetadataReadError::OutsideSpan)?;
        if !span.range().len().is_multiple_of(width_u64) {
            return Err(MetadataReadError::MalformedRecord {
                offset: span.range().offset(),
            });
        }
        Ok(Some(Self {
            view,
            section: span,
            free,
            width,
            ordinal: 0,
            count: span.range().len() / width_u64,
            admitted: 0,
            failed: false,
        }))
    }

    /// Reads one complete fixed record before exposing its unmodified words.
    ///
    /// # Errors
    /// Refuses source mismatches, malformed extents, cancellation, budgets or source failures.
    pub fn next_record(
        &mut self,
        access: &mut MetadataAccess<'source, '_>,
    ) -> Result<Option<MetadataTableRow<'source>>, MetadataReadError> {
        if self.failed {
            return Err(MetadataReadError::FailedIteration);
        }
        if !self.section.belongs_to(access.view().source()) {
            return Err(MetadataReadError::ForeignSource);
        }
        access.check_cancelled()?;
        if self.ordinal == self.count {
            return Ok(None);
        }
        access.charge(0, 1)?;
        let result = self.advance(access);
        if result.is_err() {
            self.failed = true;
        }
        result.map(Some)
    }

    fn advance(
        &mut self,
        access: &mut MetadataAccess<'source, '_>,
    ) -> Result<MetadataTableRow<'source>, MetadataReadError> {
        let width = u64::try_from(self.width).map_err(|_| MetadataReadError::OutsideSpan)?;
        let position = self
            .ordinal
            .checked_mul(width)
            .ok_or(MetadataReadError::OutsideSpan)?;
        let raw = self.section.subspan(position, width)?;
        let mut bytes = [0_u8; FREE_RECORD_BYTES];
        let buffer = bytes
            .get_mut(..self.width)
            .ok_or(MetadataReadError::OutsideSpan)?;
        access.read_into(raw, 0, buffer)?;
        let mut reader = ByteReader::with_base(buffer, raw.range().offset());
        let result = if self.free {
            self.free_value(&mut reader)
        } else {
            self.install_value(&mut reader)
        };
        let value = result.map_err(|source| MetadataReadError::Parse {
            offset: raw.range().offset(),
            source: Box::new(source),
        })?;
        let row = MetadataTableRow {
            ordinal: self.ordinal,
            raw,
            value,
        };
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(MetadataReadError::OutsideSpan)?;
        Ok(row)
    }

    fn free_value(
        &self,
        reader: &mut ByteReader<'_>,
    ) -> Result<MetadataTableValue, p4kx_foundation::ParseError> {
        Ok(MetadataTableValue::FreeRange(MetadataFreeValue {
            extent: MetadataRegion::of(
                reader.read_u64()?,
                reader.read_u64()?,
                self.view.source_len(),
            ),
        }))
    }

    fn install_value(
        &mut self,
        reader: &mut ByteReader<'_>,
    ) -> Result<MetadataTableValue, p4kx_foundation::ParseError> {
        let written = reader.read_u64()?;
        let reserved = if self.width == ZIP_PROGRESS_BYTES {
            Some(reader.read_u32()?)
        } else {
            None
        };
        let bound = match self.view.entries().get(self.admitted) {
            Some(entry) if entry.metadata().original_directory_ordinal() == Some(self.ordinal) => {
                self.admitted += 1;
                if written <= entry.compressed_size() {
                    MetadataProgressBound::WithinStoredSize
                } else {
                    MetadataProgressBound::ExceedsStoredSize
                }
            }
            _ => MetadataProgressBound::Unavailable,
        };
        Ok(MetadataTableValue::Installation(MetadataInstallValue {
            written,
            reserved,
            bound,
        }))
    }
}
