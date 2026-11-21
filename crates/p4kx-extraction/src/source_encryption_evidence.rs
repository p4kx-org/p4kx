// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    EncryptionDeclarationEvidence, EncryptionDeclarationSource, EncryptionFieldEvidence,
    EncryptionFormatEvidence, EntryRecordFormat, SourceEncryption,
};

use crate::{SourceDeclarationKind, SourceEncryptionState, SourceRecordFormat, SourceValue};

/// Partial descriptive declaration fields without an original runtime binding.
#[derive(Debug, Clone, Copy)]
pub struct SourceDeclarationEvidence(EncryptionDeclarationEvidence);

impl SourceDeclarationEvidence {
    /// Keeps the canonical possibilities for available declaration fields.
    /// Absent, unreadable and malformed fields retain their distinct canonical meanings.
    #[must_use]
    pub fn new(
        kind: SourceDeclarationKind,
        block_length: SourceValue<u16>,
        word: SourceValue<u16>,
    ) -> Self {
        let source = match kind {
            SourceDeclarationKind::CigExtraBlock => EncryptionDeclarationSource::CigExtraBlock,
            SourceDeclarationKind::P4kV2Field => EncryptionDeclarationSource::P4kV2Field,
            SourceDeclarationKind::Unknown => {
                return Self(EncryptionDeclarationEvidence::unsupported());
            }
        };
        Self(EncryptionDeclarationEvidence::new(
            source,
            field(block_length),
            field(word),
        ))
    }

    /// Retains a declared malformed or unsupported occurrence as known unknown evidence.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self(EncryptionDeclarationEvidence::unsupported())
    }
    /// Keeps all declaration states possible for unavailable full detail or one unavailable slot.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self(EncryptionDeclarationEvidence::unavailable())
    }
}

impl SourceEncryptionState {
    /// Checks descriptive stored state against every available partial declaration and header fact.
    /// An omitted format is unavailable; a known unsupported format retains its canonical rule.
    #[must_use]
    pub fn is_possible_from_evidence(
        self,
        format: Option<SourceRecordFormat>,
        zip_flags: SourceValue<u16>,
        declarations: impl IntoIterator<Item = SourceDeclarationEvidence>,
    ) -> bool {
        let format = match format {
            None => EncryptionFormatEvidence::Unavailable,
            Some(SourceRecordFormat::ZipCentralDirectory) => {
                EncryptionFormatEvidence::Known(EntryRecordFormat::ZipCentralDirectory)
            }
            Some(SourceRecordFormat::P4kV2) => {
                EncryptionFormatEvidence::Known(EntryRecordFormat::P4kV2)
            }
            Some(SourceRecordFormat::Unknown) => EncryptionFormatEvidence::Unsupported,
        };
        SourceEncryption::possible_from_evidence(
            format,
            field(zip_flags),
            declarations.into_iter().map(|declaration| declaration.0),
        )
        .any(|state| Self::of(state) == self)
    }
}

fn field<T>(value: SourceValue<T>) -> EncryptionFieldEvidence<T> {
    match value {
        SourceValue::Present(value) => EncryptionFieldEvidence::Present(value),
        SourceValue::Absent => EncryptionFieldEvidence::Absent,
        SourceValue::Unreadable => EncryptionFieldEvidence::Unavailable,
        SourceValue::Null | SourceValue::Unsupported | SourceValue::Invalid => {
            EncryptionFieldEvidence::Malformed
        }
    }
}

#[cfg(test)]
#[path = "source_encryption_evidence_tests.rs"]
mod tests;
