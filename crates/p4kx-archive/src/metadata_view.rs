// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::atomic::AtomicBool;

use crate::archive_metadata::{ArchiveMetadata, ArchiveMetadataStatus};
use crate::available_metadata_span::AvailableMetadataSpan;
use crate::entry::Entry;
use crate::metadata_access::{MetadataAccess, MetadataCost};
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_records::MetadataRecords;
use crate::metadata_region::{MetadataRegion, MetadataRegionStatus};
use crate::metadata_section::MetadataSection;
use crate::metadata_span::MetadataSpan;
use crate::metadata_table_records::MetadataTableRecords;
use crate::source::Source;

/// Metadata-only access to one source and its physical Entries without snapshotting source bytes.
#[derive(Clone, Copy)]
pub struct MetadataView<'source> {
    source: &'source Source,
    metadata: &'source ArchiveMetadata,
    entries: &'source [Entry],
}

impl<'source> MetadataView<'source> {
    pub(crate) const fn new(
        source: &'source Source,
        metadata: &'source ArchiveMetadata,
        entries: &'source [Entry],
    ) -> Self {
        Self {
            source,
            metadata,
            entries,
        }
    }

    /// Returns the owning source length, not the backing Archive file length.
    #[must_use]
    pub const fn source_len(self) -> u64 {
        self.source.len()
    }

    /// Returns the source declarations retained by the existing layout reader.
    #[must_use]
    pub const fn metadata(self) -> &'source ArchiveMetadata {
        self.metadata
    }

    /// Borrows the caller's shared budget adapter without creating or resetting counters.
    pub fn access<'budget>(
        self,
        charge: &'budget mut dyn FnMut(MetadataCost) -> io::Result<()>,
        cancelled: &'budget AtomicBool,
    ) -> MetadataAccess<'source, 'budget> {
        MetadataAccess::new(self, charge, cancelled)
    }

    /// Returns raw declaration/availability facts for one applicable metadata section.
    ///
    /// # Errors
    /// Refuses an unknown recovered layout instead of returning a successful empty layout.
    pub fn region(
        self,
        section: MetadataSection,
    ) -> Result<Option<MetadataRegion>, MetadataReadError> {
        if self.metadata.status() == ArchiveMetadataStatus::Recovered {
            return Err(MetadataReadError::UnavailableLayout);
        }
        self.metadata.region(section)
    }

    /// Returns a bound span only when the complete declaration fits this source.
    ///
    /// # Errors
    /// Refuses unavailable layouts and incomplete metadata declarations.
    pub fn span(
        self,
        section: MetadataSection,
    ) -> Result<Option<MetadataSpan<'source>>, MetadataReadError> {
        let Some(region) = self.region(section)? else {
            return Ok(None);
        };
        if region.status() != MetadataRegionStatus::Complete {
            return Err(MetadataReadError::MalformedRecord {
                offset: region.offset(),
            });
        }
        Ok(region
            .available()
            .map(|range| MetadataSpan::bind(self.source, range)))
    }

    /// Returns explicitly partial available bytes separately from their declaration status.
    ///
    /// # Errors
    /// Refuses an unavailable recovered global layout.
    pub fn available_span(
        self,
        section: MetadataSection,
    ) -> Result<Option<AvailableMetadataSpan<'source>>, MetadataReadError> {
        let Some(region) = self.region(section)? else {
            return Ok(None);
        };
        Ok(region.available().map(|range| AvailableMetadataSpan {
            span: MetadataSpan::bind(self.source, range),
            status: region.status(),
        }))
    }

    /// Returns the actual original directory-record bytes of an admitted physical Entry.
    ///
    /// # Errors
    /// Refuses a foreign Entry or a record extent outside this owning source.
    pub fn entry_record(self, entry: &Entry) -> Result<MetadataSpan<'source>, MetadataReadError> {
        if !self
            .entries
            .get(entry.index())
            .is_some_and(|own| std::ptr::eq(own, entry))
        {
            return Err(MetadataReadError::ForeignEntry);
        }
        let range = entry.metadata().record_range();
        if range.end() > self.source.len() {
            return Err(MetadataReadError::OutsideSpan);
        }
        Ok(MetadataSpan::bind(self.source, range))
    }

    /// Creates a source-record cursor without losing omitted folders or inventing recovered ordinals.
    ///
    /// # Errors
    /// Refuses invalid directory extents or overflowing declarations.
    pub fn records(self) -> Result<MetadataRecords<'source>, MetadataReadError> {
        MetadataRecords::new(self)
    }

    /// Returns progress records only where a recognised declaration establishes the table.
    ///
    /// # Errors
    /// Refuses recovered or malformed global table declarations.
    pub fn installation_records(
        self,
    ) -> Result<Option<MetadataTableRecords<'source>>, MetadataReadError> {
        MetadataTableRecords::new(self, MetadataSection::Installation)
    }

    /// Returns original free records without loading an entire declared list into memory.
    ///
    /// # Errors
    /// Refuses recovered or malformed global table declarations.
    pub fn free_records(self) -> Result<Option<MetadataTableRecords<'source>>, MetadataReadError> {
        MetadataTableRecords::new(self, MetadataSection::FreeRanges)
    }

    pub(crate) const fn source(self) -> &'source Source {
        self.source
    }
    pub(crate) const fn entries(self) -> &'source [Entry] {
        self.entries
    }
}
