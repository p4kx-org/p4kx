// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_archive::Archive;
use p4kx_gfx::Movie;
use p4kx_localization::Language;
use rayon::prelude::*;

use crate::LocalizationEntry;
use crate::entry_path::same_entry_path;
use crate::output_file::read_error;
use crate::sound::read_capped;
use crate::source_protection::SourceProtection;
use crate::{ConversionError, UnknownEntries};

/// The largest GFx or SWF file read into memory, far above the largest movie of a real Archive.
pub const MAX_UI_MOVIE_LEN: u64 = 64 << 20;

/// Where a UI movie was read from, to find the image files it refers to.
#[derive(Debug, Clone)]
pub(crate) enum Origin<'a> {
    /// An Entry of an Archive.
    Entry(&'a Archive),
    /// A file on disk.
    File(PathBuf),
}

/// A GFx or SWF UI movie read from an Entry of an Archive or from a file on disk.
#[derive(Debug)]
pub struct UiMovie<'a> {
    protection: SourceProtection,
    name: String,
    origin: Origin<'a>,
    movie: Movie,
    texts: Option<LocalizationEntry>,
    font_language: Option<Language>,
}

impl<'a> UiMovie<'a> {
    /// Reads and parses the movie Entry at `path`, matched with either separator and ignoring
    /// ASCII case; its external images are looked up in `archive`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::UiMovieTooLarge`] when it exceeds [`MAX_UI_MOVIE_LEN`],
    /// [`ConversionError::UiMovie`] when it is no valid movie, and another [`ConversionError`]
    /// when it cannot be read.
    pub fn from_archive(archive: &'a Archive, path: &str) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(Some(archive.path()));
        let entry = archive
            .entries()
            .par_iter()
            .find_first(|entry| same_entry_path(entry.path(), path))
            .ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        let name = entry.path().to_owned();
        let reader = archive.open_entry(entry)?;
        let bytes =
            read_capped(reader, entry.uncompressed_size(), MAX_UI_MOVIE_LEN).map_err(|source| {
                ConversionError::ReadEntry {
                    entry: name.clone(),
                    source,
                }
            })?;
        Self::parse(name, Origin::Entry(archive), bytes, protection)
    }

    /// Reads and parses the movie file at `path`; its external images are looked up on disk
    /// relative to it.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::UiMovieTooLarge`] when it exceeds [`MAX_UI_MOVIE_LEN`],
    /// [`ConversionError::UiMovie`] when it is no valid movie, and another [`ConversionError`]
    /// when it cannot be read.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        let file = File::open(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            }
        })?;
        let len = file
            .metadata()
            .map_err(|source| read_error(path, source))?
            .len();
        let bytes =
            read_capped(file, len, MAX_UI_MOVIE_LEN).map_err(|source| read_error(path, source))?;
        Self::parse(
            path.display().to_string(),
            Origin::File(path.to_path_buf()),
            bytes,
            SourceProtection::new(None),
        )
    }

    fn parse(
        name: String,
        origin: Origin<'a>,
        bytes: Option<Vec<u8>>,
        protection: SourceProtection,
    ) -> Result<Self, ConversionError> {
        let Some(bytes) = bytes else {
            return Err(ConversionError::UiMovieTooLarge {
                movie: name,
                limit: MAX_UI_MOVIE_LEN,
            });
        };
        match Movie::parse(bytes) {
            Ok(movie) => Ok(Self {
                protection,
                name,
                origin,
                movie,
                texts: None,
                font_language: None,
            }),
            Err(source) => Err(ConversionError::UiMovie {
                movie: name,
                source,
            }),
        }
    }

    /// Sets the texts that localisation keys (`@key`) of text fields resolve in when the movie is
    /// rendered; a key they do not hold is drawn as it is and listed in the report.
    #[must_use]
    pub fn with_texts(mut self, texts: LocalizationEntry) -> Self {
        self.texts = Some(texts);
        self
    }

    /// Sets the language whose localisation folder the fonts that the movie imports are looked
    /// for in, after the folders of the UI; it overrides the language of the texts.
    #[must_use]
    pub const fn with_font_language(mut self, language: Language) -> Self {
        self.font_language = Some(language);
        self
    }

    /// Returns the language the imported fonts are looked for in: the one set with
    /// [`UiMovie::with_font_language`], else that of the texts, else English.
    #[must_use]
    pub fn font_language(&self) -> Language {
        self.font_language
            .or_else(|| self.texts.as_ref().map(LocalizationEntry::language))
            .unwrap_or(Language::English)
    }

    /// Returns the texts localisation keys resolve in, if set.
    pub(crate) const fn texts(&self) -> Option<&LocalizationEntry> {
        self.texts.as_ref()
    }

    /// Returns the name of the movie: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the title of a document made from the movie: its Entry path or the name of its
    /// file, never the directory it lies in.
    #[must_use]
    pub(crate) fn title(&self) -> Cow<'_, str> {
        match &self.origin {
            Origin::File(path) => path
                .file_name()
                .map_or_else(|| Cow::Borrowed(self.name.as_str()), OsStr::to_string_lossy),
            Origin::Entry(_) => Cow::Borrowed(&self.name),
        }
    }

    /// Returns the parsed movie.
    #[must_use]
    pub const fn movie(&self) -> &Movie {
        &self.movie
    }

    /// Returns where the movie was read from.
    pub(crate) const fn origin(&self) -> &Origin<'a> {
        &self.origin
    }

    pub(crate) fn write_protection(
        &self,
    ) -> Result<Option<&crate::ArchiveWriteProtection>, ConversionError> {
        self.protection.context()
    }
}
