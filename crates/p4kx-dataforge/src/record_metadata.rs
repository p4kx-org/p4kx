// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::rows::NO_STRING;
use crate::{DataForgeError, Record};

/// The exact declaration of one original Record-table text field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordText<'source> {
    /// The Record layout has no word for this field.
    Absent,
    /// The stored word is the format's no-string sentinel.
    Null,
    /// The original string, including an explicitly stored empty string.
    Present(&'source str),
}

impl<'source> RecordText<'source> {
    /// Visits the exact declaration state without collapsing an empty string.
    pub fn fold<Output>(
        self,
        absent: impl FnOnce() -> Output,
        null: impl FnOnce() -> Output,
        present: impl FnOnce(&'source str) -> Output,
    ) -> Output {
        match self {
            Self::Absent => absent(),
            Self::Null => null(),
            Self::Present(value) => present(value),
        }
    }
}

/// Borrowed original text metadata captured during one successful canonical Record read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordMetadata<'source> {
    file_name: &'source str,
    name: RecordText<'source>,
    tag: RecordText<'source>,
}

impl<'source> RecordMetadata<'source> {
    pub(crate) const fn new(
        file_name: &'source str,
        name: RecordText<'source>,
        tag: RecordText<'source>,
    ) -> Self {
        Self {
            file_name,
            name,
            tag,
        }
    }

    /// Returns the exact stored filename, including its extension and original spelling.
    #[must_use]
    pub const fn file_name(&self) -> &'source str {
        self.file_name
    }

    /// Returns the raw Record-name declaration independently of the legacy empty-name default.
    #[must_use]
    pub const fn name(&self) -> RecordText<'source> {
        self.name
    }

    /// Returns the raw metadata tag independently of the legacy empty-tag filter.
    #[must_use]
    pub const fn metadata_tag(&self) -> RecordText<'source> {
        self.tag
    }

    pub(crate) const fn legacy_name(&self) -> &'source str {
        match self.name {
            RecordText::Present(name) => name,
            RecordText::Absent | RecordText::Null => "",
        }
    }

    pub(crate) const fn legacy_tag(&self) -> Option<&'source str> {
        match self.tag {
            RecordText::Present(tag) if !tag.is_empty() => Some(tag),
            RecordText::Absent | RecordText::Null | RecordText::Present(_) => None,
        }
    }
}

impl<'source> Record<'source> {
    /// Reads the original name declaration, preserving the no-string sentinel.
    ///
    /// # Errors
    /// Returns the original invalid-string error when its stored offset cannot be read.
    pub fn stored_name(&self) -> Result<RecordText<'source>, DataForgeError> {
        self.stored_blob(Some(self.def.name))
    }

    /// Reads the exact stored filename without normalizing its extension or spelling.
    ///
    /// # Errors
    /// Returns the original invalid-string error when its stored offset cannot be read.
    pub fn stored_file_name(&self) -> Result<&'source str, DataForgeError> {
        self.forge.text.get(self.def.file_name)
    }

    /// Reads the raw Record metadata tag, preserving layout absence and the no-string sentinel.
    ///
    /// # Errors
    /// Returns the original invalid-string error when its stored offset cannot be read.
    pub fn stored_tag(&self) -> Result<RecordText<'source>, DataForgeError> {
        self.stored_blob(self.def.tag)
    }

    fn stored_blob(&self, offset: Option<u32>) -> Result<RecordText<'source>, DataForgeError> {
        match offset {
            None => Ok(RecordText::Absent),
            Some(NO_STRING) => Ok(RecordText::Null),
            Some(offset) => self.forge.blob.get(offset).map(RecordText::Present),
        }
    }
}

#[cfg(test)]
#[path = "record_metadata_tests.rs"]
mod tests;
