// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DeclaredEncryption, EntryMetadata, EntryRecordFormat};

const ZIP_ENCRYPTED_FLAG: u16 = 1;
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
        match metadata.encryption_declarations().state() {
            DeclaredEncryption::Encrypted => Self::Encrypted,
            DeclaredEncryption::Unknown => Self::Unknown,
            DeclaredEncryption::Unencrypted => match metadata.record_format() {
                EntryRecordFormat::P4kV2 => Self::Unencrypted,
                EntryRecordFormat::ZipCentralDirectory => {
                    let clear = metadata.zip_header().is_some_and(|header| {
                        header.general_purpose_flags()
                            & (ZIP_ENCRYPTED_FLAG | ZIP_STRONG_ENCRYPTION_FLAG)
                            == 0
                    });
                    if clear {
                        Self::Unencrypted
                    } else {
                        Self::Unknown
                    }
                }
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
