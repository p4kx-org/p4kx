// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::archive_metadata::ArchiveLayoutKind;
use crate::directory_v2_trailer::RECORD_LEN;
use crate::entry_record_format::EntryRecordFormat;
use crate::metadata_access::MetadataAccess;
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_record::MetadataRecord;
use crate::metadata_section::MetadataSection;
use crate::metadata_span::MetadataSpan;
use crate::metadata_view::MetadataView;

/// Bounded iteration over declared source records or explicitly recovered surviving records.
pub struct MetadataRecords<'source> {
    view: MetadataView<'source>,
    directory: Option<MetadataSpan<'source>>,
    format: EntryRecordFormat,
    count: u64,
    ordinal: u64,
    byte_offset: u64,
    admitted: usize,
    recovered: bool,
    failed: bool,
}

impl<'source> MetadataRecords<'source> {
    pub(crate) fn new(view: MetadataView<'source>) -> Result<Self, MetadataReadError> {
        let recovered = view.metadata().layout() == ArchiveLayoutKind::RecoveredUnknown;
        let (format, count) = if let Some(zip) = view.metadata().zip_directory() {
            (EntryRecordFormat::ZipCentralDirectory, zip.count())
        } else if let Some(v2) = view.metadata().v2_directory() {
            (
                EntryRecordFormat::P4kV2,
                v2.base_count()
                    .checked_add(v2.extension_count())
                    .ok_or(MetadataReadError::OutsideSpan)?,
            )
        } else {
            (
                EntryRecordFormat::ZipCentralDirectory,
                u64::try_from(view.entries().len()).map_err(|_| MetadataReadError::OutsideSpan)?,
            )
        };
        let directory = if recovered {
            None
        } else {
            view.span(MetadataSection::Directory)?
        };
        Ok(Self {
            view,
            directory,
            format,
            count,
            ordinal: 0,
            byte_offset: 0,
            admitted: 0,
            recovered,
            failed: false,
        })
    }

    /// Advances through one source record using the same shared budget and cancellation flag.
    ///
    /// # Errors
    /// Retains read/parser failures and refuses foreign access or continuation after failure.
    pub fn next_record(
        &mut self,
        access: &mut MetadataAccess<'source, '_>,
    ) -> Result<Option<MetadataRecord<'source>>, MetadataReadError> {
        if self.failed {
            return Err(MetadataReadError::FailedIteration);
        }
        if !std::ptr::eq(self.view.source(), access.view().source()) {
            return Err(MetadataReadError::ForeignSource);
        }
        access.check_cancelled()?;
        let result = self.advance(access);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn advance(
        &mut self,
        access: &mut MetadataAccess<'source, '_>,
    ) -> Result<Option<MetadataRecord<'source>>, MetadataReadError> {
        if self.ordinal == self.count {
            return self.finish();
        }
        access.charge(0, 1)?;
        let admitted_index = self.admitted_index()?;
        let record = if self.recovered {
            let index =
                usize::try_from(self.ordinal).map_err(|_| MetadataReadError::OutsideSpan)?;
            let entry = self
                .view
                .entries()
                .get(index)
                .ok_or(MetadataReadError::OutsideSpan)?;
            let span = self.view.entry_record(entry)?;
            crate::metadata_zip_record::read(access, span, 0, None, Some(entry.index()))?
        } else {
            let directory = self.directory.ok_or(MetadataReadError::UnavailableLayout)?;
            match self.format {
                EntryRecordFormat::ZipCentralDirectory => crate::metadata_zip_record::read(
                    access,
                    directory,
                    self.byte_offset,
                    Some(self.ordinal),
                    admitted_index,
                )?,
                EntryRecordFormat::P4kV2 => MetadataRecord {
                    raw: directory.subspan(self.byte_offset, RECORD_LEN as u64)?,
                    format: self.format,
                    ordinal: Some(self.ordinal),
                    admitted_index,
                    name: None,
                    extra: None,
                    comment: None,
                },
            }
        };
        self.byte_offset = self
            .byte_offset
            .checked_add(record.raw().range().len())
            .ok_or(MetadataReadError::OutsideSpan)?;
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(MetadataReadError::OutsideSpan)?;
        Ok(Some(record))
    }

    fn finish(&self) -> Result<Option<MetadataRecord<'source>>, MetadataReadError> {
        if let Some(directory) = self.directory {
            let len = directory
                .range()
                .len()
                .checked_sub(self.byte_offset)
                .ok_or(MetadataReadError::OutsideSpan)?;
            if len != 0 {
                let tail = directory.subspan(self.byte_offset, len)?;
                return Err(MetadataReadError::UnparsedDirectoryTail {
                    offset: tail.range().offset(),
                    len,
                });
            }
        }
        Ok(None)
    }

    fn admitted_index(&mut self) -> Result<Option<usize>, MetadataReadError> {
        if self.recovered {
            return Ok(None);
        }
        let Some(entry) = self.view.entries().get(self.admitted) else {
            return Ok(None);
        };
        if entry.metadata().original_directory_ordinal() == Some(self.ordinal) {
            self.admitted = self
                .admitted
                .checked_add(1)
                .ok_or(MetadataReadError::OutsideSpan)?;
            Ok(Some(entry.index()))
        } else {
            Ok(None)
        }
    }
}
