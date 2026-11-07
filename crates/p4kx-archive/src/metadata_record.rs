// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entry_record_format::EntryRecordFormat;
use crate::metadata_extras::MetadataExtras;
use crate::metadata_read_error::MetadataReadError;
use crate::metadata_span::MetadataSpan;

const V2_SIGNATURE_OFFSET: u64 = 42;
const V2_SIGNATURE_LEN: u64 = 128;
const V2_DIGEST_OFFSET: u64 = 172;
const V2_DIGEST_LEN: u64 = 32;

/// One original directory record, including records omitted by nested admission.
#[derive(Debug, Clone, Copy)]
pub struct MetadataRecord<'source> {
    pub(crate) raw: MetadataSpan<'source>,
    pub(crate) format: EntryRecordFormat,
    pub(crate) ordinal: Option<u64>,
    pub(crate) admitted_index: Option<usize>,
    pub(crate) name: Option<MetadataSpan<'source>>,
    pub(crate) extra: Option<MetadataSpan<'source>>,
    pub(crate) comment: Option<MetadataSpan<'source>>,
}

impl<'source> MetadataRecord<'source> {
    /// Returns original record bytes without cloning its signature or variable data.
    #[must_use]
    pub const fn raw(self) -> MetadataSpan<'source> {
        self.raw
    }
    /// Returns the known physical record format.
    #[must_use]
    pub const fn format(self) -> EntryRecordFormat {
        self.format
    }
    /// Returns the original table position, unavailable for recovery-scanned headers.
    #[must_use]
    pub const fn original_directory_ordinal(self) -> Option<u64> {
        self.ordinal
    }
    /// Returns the admitted physical index, absent for omitted folder records.
    #[must_use]
    pub const fn admitted_entry_index(self) -> Option<usize> {
        self.admitted_index
    }
    /// Returns ZIP path bytes without UTF-8 decoding or separator normalisation.
    #[must_use]
    pub const fn zip_name(self) -> Option<MetadataSpan<'source>> {
        self.name
    }
    /// Returns ZIP extra bytes in their original order.
    #[must_use]
    pub const fn zip_extra(self) -> Option<MetadataSpan<'source>> {
        self.extra
    }
    /// Returns ZIP comment bytes, including an explicitly empty comment.
    #[must_use]
    pub const fn zip_comment(self) -> Option<MetadataSpan<'source>> {
        self.comment
    }

    /// Creates an ordered extra cursor without allocating or rereading field bodies.
    #[must_use]
    pub fn extras(self) -> Option<MetadataExtras<'source>> {
        self.extra.map(MetadataExtras::new)
    }

    /// Returns all 128 stored v2 signature bytes without a cryptographic interpretation.
    ///
    /// # Errors
    /// Rejects a malformed fixed record extent.
    pub fn v2_signature(self) -> Result<Option<MetadataSpan<'source>>, MetadataReadError> {
        match self.format {
            EntryRecordFormat::P4kV2 => self
                .raw
                .subspan(V2_SIGNATURE_OFFSET, V2_SIGNATURE_LEN)
                .map(Some),
            _ => Ok(None),
        }
    }

    /// Returns all 32 stored v2 digest-slot bytes, including an all-zero slot.
    ///
    /// # Errors
    /// Rejects a malformed fixed record extent.
    pub fn v2_stored_digest(self) -> Result<Option<MetadataSpan<'source>>, MetadataReadError> {
        match self.format {
            EntryRecordFormat::P4kV2 => self.raw.subspan(V2_DIGEST_OFFSET, V2_DIGEST_LEN).map(Some),
            _ => Ok(None),
        }
    }
}
