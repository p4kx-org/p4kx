// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_archive::{Archive, ArchiveError, Entry, EntryOrigin};
use p4kx_localization::{GlobalIni, IniText, Language, LocalizationError, LocalizationLimits};
use rayon::prelude::*;
use thiserror::Error;

use crate::entry_path::same_entry_path;

const INITIAL_RESERVE: u64 = 16 << 20;

/// The text of a language's `global.ini` Entry, read into memory and decoded to UTF-8.
#[derive(Debug)]
pub struct LocalizationEntry {
    language: Language,
    path: String,
    text: IniText<'static>,
    original_byte_len: u64,
    origin: Option<EntryOrigin>,
    limits: LocalizationLimits,
}

impl LocalizationEntry {
    /// Reads the `global.ini` of `language` with the default unrestricted policy.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::read_with_limits`].
    pub fn read(archive: &Archive, language: Language) -> Result<Self, LocalizationLoadError> {
        Self::read_with_limits(archive, language, LocalizationLimits::UNLIMITED)
    }

    /// Reads the language Entry with the caller's limits, retained for subsequent indexing.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationLoadError::Missing`] when the Archive holds no such Entry,
    /// [`LocalizationLoadError::TooLarge`] when its declared uncompressed size exceeds the
    /// source-byte limit, and another [`LocalizationLoadError`] when it cannot be read or decoded.
    pub fn read_with_limits(
        archive: &Archive,
        language: Language,
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationLoadError> {
        let entry =
            Self::locate(archive, language).ok_or(LocalizationLoadError::Missing { language })?;
        Self::read_entry_with_limits(archive, language, entry, limits)
    }

    pub(crate) fn read_entry_with_limits(
        archive: &Archive,
        language: Language,
        entry: &Entry,
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationLoadError> {
        let path = entry.path().to_owned();
        let size = entry.uncompressed_size();
        if let Some(limit) = limits.source_bytes().filter(|&limit| size > limit) {
            return Err(LocalizationLoadError::TooLarge { path, size, limit });
        }
        let mut bytes = Vec::new();
        let reserve = usize::try_from(size.min(INITIAL_RESERVE)).unwrap_or_default();
        if bytes.try_reserve_exact(reserve).is_err() {
            return Err(LocalizationLoadError::OutOfMemory { path, size });
        }
        let read = archive
            .open_entry(entry)
            .map_err(LocalizationLoadError::Archive)?
            .reject_overlong()
            .read_to_end(&mut bytes);
        if let Err(source) = read {
            return Err(LocalizationLoadError::Read { path, source });
        }
        let original_byte_len = bytes.len() as u64;
        let text = IniText::decode_vec_with_limits(bytes, limits).map_err(|source| {
            LocalizationLoadError::Decode {
                path: path.clone(),
                source,
            }
        })?;
        Ok(Self {
            language,
            path,
            text,
            original_byte_len,
            origin: archive.entry_origin(entry),
            limits,
        })
    }

    /// Returns the `global.ini` Entry of `language`, or `None` when the Archive holds none.
    #[must_use]
    pub fn locate(archive: &Archive, language: Language) -> Option<&Entry> {
        let wanted = language.global_ini_path();
        archive
            .entries()
            .par_iter()
            .find_first(|entry| same_entry_path(entry.path(), wanted))
    }

    /// Returns the languages the Archive holds a `global.ini` Entry for, in the order of
    /// [`Language::ALL`].
    #[must_use]
    pub fn languages(archive: &Archive) -> Vec<Language> {
        let found: Vec<Language> = archive
            .entries()
            .par_iter()
            .filter_map(|entry| Language::from_global_ini_path(entry.path()))
            .collect();
        Language::ALL
            .into_iter()
            .filter(|language| found.contains(language))
            .collect()
    }

    /// Returns the language.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Returns the path of the Entry the file was read from.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the actual language Entry and its complete containing-input context.
    #[must_use]
    pub const fn origin(&self) -> Option<&EntryOrigin> {
        self.origin.as_ref()
    }

    pub(crate) const fn original_byte_len(&self) -> u64 {
        self.original_byte_len
    }

    /// Returns the decoded text of the file.
    #[must_use]
    pub const fn text(&self) -> &IniText<'static> {
        &self.text
    }

    /// Indexes the file's texts by localisation key.
    ///
    /// # Errors
    ///
    /// Returns a [`LocalizationError`] when indexing exceeds the retained limits or the index
    /// cannot be allocated.
    pub fn parse(&self) -> Result<GlobalIni<'_>, LocalizationError> {
        self.text.index_with_limits(self.limits)
    }
}

/// The `global.ini` of a language cannot be found or read.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LocalizationLoadError {
    /// The Archive holds no `global.ini` for the language.
    #[error("the Archive holds no global.ini for the language {language}")]
    Missing {
        /// The language.
        language: Language,
    },
    /// The Entry's declared size exceeds the caller's source-byte limit.
    #[error("the Entry {path} claims {size} bytes, more than the limit of {limit} bytes")]
    TooLarge {
        /// The Entry path.
        path: String,
        /// The uncompressed size the Archive gives.
        size: u64,
        /// The largest size that is read.
        limit: u64,
    },
    /// The memory to read the Entry into cannot be reserved.
    #[error("cannot reserve the memory to read the {size} bytes of the Entry {path}")]
    OutOfMemory {
        /// The Entry path.
        path: String,
        /// The uncompressed size the Archive gives.
        size: u64,
    },
    /// The Entry's bytes cannot be decoded to text.
    #[error("cannot decode the Entry {path}")]
    Decode {
        /// The Entry path.
        path: String,
        /// The failed decoding.
        #[source]
        source: LocalizationError,
    },
    /// The Entry's text cannot be indexed.
    #[error("cannot index the Entry {path}")]
    Index {
        /// The Entry path.
        path: String,
        /// The failed indexing.
        #[source]
        source: LocalizationError,
    },
    /// The Entry cannot be opened in the Archive.
    #[error(transparent)]
    Archive(ArchiveError),
    /// Reading, decrypting or decompressing the Entry failed, or its data is shorter or longer
    /// than its stated size.
    #[error("cannot read the Entry {path}")]
    Read {
        /// The Entry path.
        path: String,
        /// The failed read.
        #[source]
        source: io::Error,
    },
}
