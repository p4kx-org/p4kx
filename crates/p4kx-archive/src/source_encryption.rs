// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DeclaredEncryption, EntryMetadata, EntryRecordFormat, ZipHeaderMetadata};

pub(crate) const ZIP_ENCRYPTED_FLAG: u16 = 1;
const ZIP_STRONG_ENCRYPTION_FLAG: u16 = 1 << 6;

/// Encryption evidence from supported source declarations, independently of payload reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceEncryption {
    /// A valid supported declaration establishes an encrypted contribution.
    Encrypted,
    /// Every relevant supported declaration establishes an unencrypted contribution.
    Unencrypted,
    /// Available declarations do not establish either state.
    Unknown,
}

impl SourceEncryption {
    pub(crate) fn of(metadata: &EntryMetadata) -> Self {
        Self::from_declarations(
            metadata.encryption_declarations().state(),
            Some(metadata.record_format()),
            metadata
                .zip_header()
                .map(ZipHeaderMetadata::general_purpose_flags),
        )
    }

    /// Reconciles descriptive declarations, directory format and ZIP flags without input authority.
    #[must_use]
    pub const fn from_declarations(
        declared: DeclaredEncryption,
        record_format: Option<EntryRecordFormat>,
        zip_flags: Option<u16>,
    ) -> Self {
        match declared {
            DeclaredEncryption::Encrypted => Self::Encrypted,
            DeclaredEncryption::Unknown => Self::Unknown,
            DeclaredEncryption::Unencrypted => match record_format {
                Some(EntryRecordFormat::P4kV2) => Self::Unencrypted,
                Some(EntryRecordFormat::ZipCentralDirectory) => match zip_flags {
                    Some(flags)
                        if flags & (ZIP_ENCRYPTED_FLAG | ZIP_STRONG_ENCRYPTION_FLAG) == 0 =>
                    {
                        Self::Unencrypted
                    }
                    _ => Self::Unknown,
                },
                None => Self::Unknown,
            },
        }
    }

    pub(crate) const fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Encrypted, _) | (_, Self::Encrypted) => Self::Encrypted,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Unencrypted, Self::Unencrypted) => Self::Unencrypted,
        }
    }
}
