// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::encryption_declarations::{ENCRYPTED_WORD, ENCRYPTION_BLOCK_LEN, UNENCRYPTED_WORD};
use crate::source_encryption::ZIP_ENCRYPTED_FLAG;
use crate::{DeclaredEncryption, EncryptionDeclarationSource, EntryRecordFormat, SourceEncryption};

/// Descriptive field evidence without collapsing source absence into unreadability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncryptionFieldEvidence<T> {
    /// The declared source value could not be read or its optional capture is unavailable.
    Unavailable,
    /// The source did not declare the field.
    Absent,
    /// A declared null, unsupported or invalid value cannot supply the required typed field.
    Malformed,
    /// The original typed value is available.
    Present(T),
}
/// Available directory-format evidence, independently of optional metadata capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncryptionFormatEvidence {
    /// No original format detail was retained.
    Unavailable,
    /// An available format has canonical interpretation rules.
    Known(EntryRecordFormat),
    /// The available format has no supported interpretation.
    Unsupported,
}

/// Possible states of one descriptive declaration without original-input authority.
#[derive(Debug, Clone, Copy)]
pub struct EncryptionDeclarationEvidence {
    states: Possibilities,
}

impl EncryptionDeclarationEvidence {
    /// Interprets available fields through the canonical declaration classifier.
    /// Only unavailable fields retain alternatives; an absent length is valid for v2 alone.
    #[must_use]
    pub fn new(
        source: EncryptionDeclarationSource,
        block_length: EncryptionFieldEvidence<u16>,
        word: EncryptionFieldEvidence<u16>,
    ) -> Self {
        let lengths = match block_length {
            EncryptionFieldEvidence::Unavailable => [None, Some(ENCRYPTION_BLOCK_LEN)],
            EncryptionFieldEvidence::Absent => [None; 2],
            EncryptionFieldEvidence::Malformed => return Self::unsupported(),
            EncryptionFieldEvidence::Present(length) => [Some(length); 2],
        };
        let words = match word {
            EncryptionFieldEvidence::Unavailable => {
                [None, Some(UNENCRYPTED_WORD), Some(ENCRYPTED_WORD)]
            }
            EncryptionFieldEvidence::Absent | EncryptionFieldEvidence::Malformed => [None; 3],
            EncryptionFieldEvidence::Present(word) => [Some(word); 3],
        };
        let mut states = Possibilities::empty();
        for length in lengths {
            for word in words {
                states.insert(DeclaredEncryption::interpret(source, length, word));
            }
        }
        Self { states }
    }
    /// Retains an available declaration without a supported interpretation.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            states: Possibilities::one(DeclaredEncryption::Unknown),
        }
    }
    /// Keeps every declaration state possible when an occurrence is unavailable.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            states: Possibilities {
                encrypted: true,
                plain: true,
                unknown: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Possibilities {
    encrypted: bool,
    plain: bool,
    unknown: bool,
}

impl Possibilities {
    const fn empty() -> Self {
        Self {
            encrypted: false,
            plain: false,
            unknown: false,
        }
    }

    const fn one(state: DeclaredEncryption) -> Self {
        let mut result = Self::empty();
        result.insert(state);
        result
    }

    const fn insert(&mut self, state: DeclaredEncryption) {
        match state {
            DeclaredEncryption::Encrypted => self.encrypted = true,
            DeclaredEncryption::Unencrypted => self.plain = true,
            DeclaredEncryption::Unknown => self.unknown = true,
        }
    }

    fn states(self) -> impl Iterator<Item = DeclaredEncryption> {
        [
            DeclaredEncryption::Encrypted,
            DeclaredEncryption::Unencrypted,
            DeclaredEncryption::Unknown,
        ]
        .into_iter()
        .filter(move |state| match state {
            DeclaredEncryption::Encrypted => self.encrypted,
            DeclaredEncryption::Unencrypted => self.plain,
            DeclaredEncryption::Unknown => self.unknown,
        })
    }

    fn consensus(self, other: Self) -> Self {
        let mut result = Self::empty();
        for first in self.states() {
            for second in other.states() {
                result.insert(DeclaredEncryption::reconcile([first, second]));
            }
        }
        result
    }

    fn source_states(self) -> impl Iterator<Item = SourceEncryption> {
        [
            SourceEncryption::Encrypted,
            SourceEncryption::Unencrypted,
            SourceEncryption::Unknown,
        ]
        .into_iter()
        .filter(move |state| match state {
            SourceEncryption::Encrypted => self.encrypted,
            SourceEncryption::Unencrypted => self.plain,
            SourceEncryption::Unknown => self.unknown,
        })
    }
}

impl SourceEncryption {
    /// Returns states compatible with all available declaration, format and ZIP evidence.
    /// Empty declarations have unknown consensus; unavailable occurrences or flags retain alternatives,
    /// while absent or malformed flags use the canonical absence rule.
    pub fn possible_from_evidence(
        format: EncryptionFormatEvidence,
        zip_flags: EncryptionFieldEvidence<u16>,
        declarations: impl IntoIterator<Item = EncryptionDeclarationEvidence>,
    ) -> impl Iterator<Item = Self> {
        let declared = declarations
            .into_iter()
            .map(|declaration| declaration.states)
            .reduce(Possibilities::consensus)
            .unwrap_or_else(|| Possibilities::one(DeclaredEncryption::Unknown));
        let formats = match format {
            EncryptionFormatEvidence::Unavailable => [
                Some(EntryRecordFormat::ZipCentralDirectory),
                Some(EntryRecordFormat::P4kV2),
                None,
            ],
            EncryptionFormatEvidence::Known(format) => [Some(format); 3],
            EncryptionFormatEvidence::Unsupported => [None; 3],
        };
        let flags = match zip_flags {
            EncryptionFieldEvidence::Unavailable => [Some(0), Some(ZIP_ENCRYPTED_FLAG)],
            EncryptionFieldEvidence::Absent | EncryptionFieldEvidence::Malformed => [None; 2],
            EncryptionFieldEvidence::Present(flags) => [Some(flags); 2],
        };
        let mut result = Possibilities::empty();
        for declaration in declared.states() {
            for format in formats {
                for flags in flags {
                    let state = match Self::from_declarations(declaration, format, flags) {
                        Self::Encrypted => DeclaredEncryption::Encrypted,
                        Self::Unencrypted => DeclaredEncryption::Unencrypted,
                        Self::Unknown => DeclaredEncryption::Unknown,
                    };
                    result.insert(state);
                }
            }
        }
        result.source_states()
    }
}
