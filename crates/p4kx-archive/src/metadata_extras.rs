// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::extra_block_header::ExtraBlockHeader;
use crate::metadata_access::MetadataAccess;
use crate::metadata_extra::{MetadataExtra, MetadataExtraStatus};
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_span::MetadataSpan;

/// Budgeted ordered extra-field framing using the same header reader as Entry interpretation.
pub struct MetadataExtras<'source> {
    section: MetadataSpan<'source>,
    position: u64,
    ordinal: u64,
    failed: bool,
}

impl<'source> MetadataExtras<'source> {
    pub(crate) const fn new(section: MetadataSpan<'source>) -> Self {
        Self {
            section,
            position: 0,
            ordinal: 0,
            failed: false,
        }
    }

    /// Advances one field without discarding malformed available bytes or duplicate IDs.
    ///
    /// # Errors
    /// Refuses foreign sources, budgets, cancellation, reads or continuation after malformed framing.
    pub fn next_field(
        &mut self,
        access: &mut MetadataAccess<'source, '_>,
    ) -> Result<Option<MetadataExtra<'source>>, MetadataReadError> {
        if self.failed {
            return Err(MetadataReadError::FailedIteration);
        }
        if !self.section.belongs_to(access.view().source()) {
            return Err(MetadataReadError::ForeignSource);
        }
        access.check_cancelled()?;
        if self.position == self.section.range().len() {
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
    ) -> Result<MetadataExtra<'source>, MetadataReadError> {
        let remaining = self
            .section
            .range()
            .len()
            .checked_sub(self.position)
            .ok_or(MetadataReadError::OutsideSpan)?;
        let raw = self.section.subspan(self.position, remaining)?;
        let mut bytes = [0_u8; 4];
        let count =
            usize::try_from(remaining.min(4)).map_err(|_| MetadataReadError::OutsideSpan)?;
        access.read_into(
            raw,
            0,
            bytes
                .get_mut(..count)
                .ok_or(MetadataReadError::OutsideSpan)?,
        )?;
        let offset = raw.range().offset();
        if count < 4 {
            return self.short_header(raw, &bytes, count);
        }
        let header = ExtraBlockHeader::read(&mut ByteReader::with_base(&bytes, offset)).map_err(
            |source| MetadataReadError::Parse {
                offset,
                source: Box::new(source),
            },
        )?;
        let status = match header.body_len() {
            None => MetadataExtraStatus::LengthBeforeHeader,
            Some(_) if u64::from(header.declared_len()) > remaining => {
                MetadataExtraStatus::TruncatedBody
            }
            Some(_) => MetadataExtraStatus::Complete,
        };
        self.finish_field(raw, header, status)
    }

    fn finish_field(
        &mut self,
        raw: MetadataSpan<'source>,
        header: ExtraBlockHeader,
        status: MetadataExtraStatus,
    ) -> Result<MetadataExtra<'source>, MetadataReadError> {
        let complete = status == MetadataExtraStatus::Complete;
        let raw = if complete {
            raw.subspan(0, u64::from(header.declared_len()))?
        } else {
            raw
        };
        let body = if header.body_len().is_some() {
            Some(raw.subspan(4, raw.range().len() - 4)?)
        } else {
            None
        };
        let field = MetadataExtra {
            ordinal: self.ordinal,
            id: Some(header.id()),
            declared_total_len: Some(header.declared_len()),
            raw,
            body,
            status,
        };
        self.failed = !complete;
        self.position = self
            .position
            .checked_add(raw.range().len())
            .ok_or(MetadataReadError::OutsideSpan)?;
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(MetadataReadError::OutsideSpan)?;
        Ok(field)
    }

    fn short_header(
        &mut self,
        raw: MetadataSpan<'source>,
        bytes: &[u8],
        count: usize,
    ) -> Result<MetadataExtra<'source>, MetadataReadError> {
        let id =
            if count >= 2 {
                Some(ByteReader::new(bytes).read_u16().map_err(|source| {
                    MetadataReadError::Parse {
                        offset: raw.range().offset(),
                        source: Box::new(source),
                    }
                })?)
            } else {
                None
            };
        self.failed = true;
        Ok(MetadataExtra {
            ordinal: self.ordinal,
            id,
            declared_total_len: None,
            raw,
            body: None,
            status: MetadataExtraStatus::ShortHeader,
        })
    }
}
