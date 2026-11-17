// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    ArchiveDirectorySource, ArchiveEntryProof, ArchiveEntrySource, ArchiveFileSource, FileSource,
    PhysicalSource, PhysicalSourceKind, PhysicalSourceRef, SourceDeclarationKind,
    SourceEncryptionDeclaration, SourceEncryptionState, SourceRecordFormat,
};
use crate::source_details::SourceDetails;
use crate::source_value::SourcedValue;
use crate::{ArchiveByteRange, DecimalU64};

impl PhysicalSource {
    /// Borrows the original containing-source references in stored order.
    #[must_use]
    pub fn ancestors(&self) -> &[PhysicalSourceRef] {
        &self.ancestors
    }

    /// Returns the observed stored encryption independently of a use's byte domain.
    #[must_use]
    pub const fn stored_encryption(&self) -> SourceEncryptionState {
        self.stored_encryption
    }

    /// Borrows the original physical-source variant and its retained facts.
    #[must_use]
    pub const fn kind(&self) -> &PhysicalSourceKind {
        &self.source
    }
}

impl ArchiveEntrySource {
    /// Returns the original directory ordinal within the immediately owning Archive.
    #[must_use]
    pub const fn entry_index(&self) -> usize {
        self.entry_index
    }

    /// Borrows the retained Archive-relative path with its original state and evidence.
    #[must_use]
    pub const fn path(&self) -> &SourcedValue<String> {
        &self.path
    }

    /// Returns the observed directory-record format.
    #[must_use]
    pub const fn record_format(&self) -> SourceRecordFormat {
        self.record_format
    }

    /// Returns the half-open byte extent relative to the immediately owning Archive.
    #[must_use]
    pub const fn record_range(&self) -> ArchiveByteRange {
        self.record_range
    }

    /// Borrows the stored ZIP flags or their original unavailable state.
    #[must_use]
    pub const fn zip_flags(&self) -> &SourcedValue<u16> {
        &self.zip_flags
    }

    /// Borrows every original encryption declaration in stored order.
    #[must_use]
    pub const fn declarations(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<SourceEncryptionDeclaration>>> {
        &self.declarations
    }
}

impl ArchiveEntryProof {
    /// Returns the original directory ordinal without narrowing its integer width.
    #[must_use]
    pub const fn entry_index(&self) -> DecimalU64 {
        self.entry_index
    }

    /// Returns the explicit reason complete stored details were not retained.
    #[must_use]
    pub const fn details(&self) -> SourceDetails {
        self.details
    }
}

impl ArchiveDirectorySource {
    /// Returns the number of admitted original directory Entries.
    #[must_use]
    pub const fn admitted_entry_count(&self) -> DecimalU64 {
        self.admitted_entry_count
    }
}

impl ArchiveFileSource {
    /// Borrows the measured whole-Archive byte length and its evidence.
    #[must_use]
    pub const fn byte_length(&self) -> &SourcedValue<DecimalU64> {
        &self.byte_length
    }
}

impl FileSource {
    /// Borrows the retained file label without assigning original P4K lineage.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Borrows the measured file byte length and its evidence.
    #[must_use]
    pub const fn byte_length(&self) -> &SourcedValue<DecimalU64> {
        &self.byte_length
    }
}

impl SourceEncryptionDeclaration {
    /// Returns the original declaration-bearing field kind.
    #[must_use]
    pub const fn kind(&self) -> SourceDeclarationKind {
        self.kind
    }

    /// Returns the expected value byte position within the immediately owning Archive.
    #[must_use]
    pub const fn value_offset(&self) -> DecimalU64 {
        self.value_offset
    }

    /// Borrows the stored total block length in bytes, including its four-byte header.
    #[must_use]
    pub const fn declared_block_length(&self) -> &SourcedValue<u16> {
        &self.declared_block_length
    }

    /// Borrows the complete stored word or its original invalid state.
    #[must_use]
    pub const fn raw_word(&self) -> &SourcedValue<u16> {
        &self.raw_word
    }

    /// Borrows available value bytes in their original stored order.
    #[must_use]
    pub const fn raw_bytes(&self) -> &SourcedValue<Vec<u8>> {
        &self.raw_bytes
    }
}
