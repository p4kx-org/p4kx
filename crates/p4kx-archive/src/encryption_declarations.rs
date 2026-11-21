// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

pub(crate) const ENCRYPTION_BLOCK_LEN: u16 = 6;
const WORD_LEN: u8 = 2;
pub(crate) const UNENCRYPTED_WORD: u16 = 0;
pub(crate) const ENCRYPTED_WORD: u16 = 1;

/// Stored CIG/v2 declaration state, without reconciling ZIP encryption flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeclaredEncryption {
    /// Every stored declaration establishes encryption.
    Encrypted,
    /// Every stored CIG/v2 declaration states the absence of encryption.
    Unencrypted,
    /// A declaration is absent, malformed, unsupported, or contradictory.
    Unknown,
}

impl DeclaredEncryption {
    /// Interprets descriptive stored fields without an original input binding.
    #[must_use]
    pub const fn interpret(
        source: EncryptionDeclarationSource,
        declared_block_len: Option<u16>,
        raw_word: Option<u16>,
    ) -> Self {
        let valid_shape = match source {
            EncryptionDeclarationSource::CigExtraBlock => {
                matches!(declared_block_len, Some(ENCRYPTION_BLOCK_LEN))
            }
            EncryptionDeclarationSource::P4kV2Field => declared_block_len.is_none(),
        };
        if !valid_shape {
            return Self::Unknown;
        }
        match raw_word {
            Some(UNENCRYPTED_WORD) => Self::Unencrypted,
            Some(ENCRYPTED_WORD) => Self::Encrypted,
            _ => Self::Unknown,
        }
    }

    /// Reconciles all declaration occurrences without giving a contradictory field precedence.
    #[must_use]
    pub fn reconcile(states: impl IntoIterator<Item = Self>) -> Self {
        let mut states = states.into_iter();
        let Some(first) = states.next() else {
            return Self::Unknown;
        };
        if states.all(|state| state == first) {
            first
        } else {
            Self::Unknown
        }
    }
}

/// The physical field supplying an encryption declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncryptionDeclarationSource {
    /// The CIG 0x5002 block in a ZIP central-directory record.
    CigExtraBlock,
    /// The encryption word in a fixed P4K v2 directory record.
    P4kV2Field,
}

/// A stored encryption field and the source coordinates required to inspect it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredEncryptionDeclaration {
    value_offset: u64,
    declared_block_len: Option<u16>,
    raw_bytes: [u8; 2],
    raw_len: u8,
    source: EncryptionDeclarationSource,
}

impl StoredEncryptionDeclaration {
    pub(crate) const fn cig(
        value_offset: u64,
        declared_block_len: u16,
        raw_value: Option<u16>,
        first_byte: Option<u8>,
    ) -> Self {
        let (raw_bytes, raw_len) = match (raw_value, first_byte) {
            (Some(value), _) => (value.to_le_bytes(), WORD_LEN),
            (None, Some(value)) => ([value, 0], 1),
            (None, None) => ([0, 0], 0),
        };
        Self {
            value_offset,
            declared_block_len: Some(declared_block_len),
            raw_bytes,
            raw_len,
            source: EncryptionDeclarationSource::CigExtraBlock,
        }
    }

    pub(crate) const fn v2(value_offset: u64, raw_value: u16) -> Self {
        Self {
            value_offset,
            declared_block_len: None,
            raw_bytes: raw_value.to_le_bytes(),
            raw_len: WORD_LEN,
            source: EncryptionDeclarationSource::P4kV2Field,
        }
    }

    /// Returns the exact physical field supplying the declaration.
    #[must_use]
    pub const fn source(&self) -> EncryptionDeclarationSource {
        self.source
    }

    /// Returns the expected value position relative to the immediately owning Archive source.
    /// Containers have their own byte origins; an empty block has no value bytes there.
    #[must_use]
    pub const fn value_offset(&self) -> u64 {
        self.value_offset
    }

    /// Returns the CIG block's stored total length, including its four-byte header.
    #[must_use]
    pub const fn declared_block_len(&self) -> Option<u16> {
        self.declared_block_len
    }

    /// Returns the available value bytes in stored order, including a short field.
    pub fn raw_value_bytes(&self) -> impl ExactSizeIterator<Item = u8> + '_ {
        self.raw_bytes
            .iter()
            .copied()
            .take(usize::from(self.raw_len))
    }

    /// Returns the complete stored word, including unsupported values.
    #[must_use]
    pub const fn raw_value(&self) -> Option<u16> {
        if self.raw_len == WORD_LEN {
            Some(u16::from_le_bytes(self.raw_bytes))
        } else {
            None
        }
    }

    /// Interprets this CIG/v2 field without interpreting ZIP flags or payload bytes.
    #[must_use]
    pub const fn state(&self) -> DeclaredEncryption {
        DeclaredEncryption::interpret(self.source, self.declared_block_len, self.raw_value())
    }
}

/// Complete encryption declarations in directory order, independently of diagnostic sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionDeclarations {
    storage: DeclarationStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeclarationStorage {
    Missing,
    Single(StoredEncryptionDeclaration),
    Multiple(Box<[StoredEncryptionDeclaration]>),
}

impl EncryptionDeclarations {
    pub(crate) const fn v2(value_offset: u64, raw_value: u16) -> Self {
        Self {
            storage: DeclarationStorage::Single(StoredEncryptionDeclaration::v2(
                value_offset,
                raw_value,
            )),
        }
    }

    /// Iterates every stored declaration in its original occurrence order.
    pub fn iter(&self) -> slice::Iter<'_, StoredEncryptionDeclaration> {
        let declarations = match &self.storage {
            DeclarationStorage::Missing => &[],
            DeclarationStorage::Single(declaration) => slice::from_ref(declaration),
            DeclarationStorage::Multiple(declarations) => declarations.as_ref(),
        };
        declarations.iter()
    }

    /// Summarizes stored CIG/v2 fields without interpreting ZIP encryption flags.
    /// A known state requires every present field to be valid and equal; absence stays unknown.
    #[must_use]
    pub fn state(&self) -> DeclaredEncryption {
        DeclaredEncryption::reconcile(self.iter().map(StoredEncryptionDeclaration::state))
    }

    pub(crate) const fn raw_v2_word(&self) -> Option<u16> {
        match &self.storage {
            DeclarationStorage::Single(declaration) => declaration.raw_value(),
            DeclarationStorage::Missing | DeclarationStorage::Multiple(_) => None,
        }
    }
}

impl<'a> IntoIterator for &'a EncryptionDeclarations {
    type Item = &'a StoredEncryptionDeclaration;
    type IntoIter = slice::Iter<'a, StoredEncryptionDeclaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Default)]
pub(crate) enum EncryptionDeclarationsBuilder {
    #[default]
    Missing,
    Single(StoredEncryptionDeclaration),
    Multiple(Vec<StoredEncryptionDeclaration>),
}

impl EncryptionDeclarationsBuilder {
    pub(crate) fn push(&mut self, declaration: StoredEncryptionDeclaration) {
        match self {
            Self::Missing => *self = Self::Single(declaration),
            Self::Single(first) => *self = Self::Multiple(vec![*first, declaration]),
            Self::Multiple(declarations) => declarations.push(declaration),
        }
    }

    pub(crate) fn finish(self) -> EncryptionDeclarations {
        let storage = match self {
            Self::Missing => DeclarationStorage::Missing,
            Self::Single(declaration) => DeclarationStorage::Single(declaration),
            Self::Multiple(declarations) => {
                DeclarationStorage::Multiple(declarations.into_boxed_slice())
            }
        };
        EncryptionDeclarations { storage }
    }
}
