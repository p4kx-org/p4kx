// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "ui_import_resolver.rs"]
mod resolver;

use std::path::Path;

pub(crate) use self::resolver::{ImportedFonts, resolve_fonts};

use crate::ui_import_match::{candidates, contained, folder_of};
use crate::ui_movie::Origin;
use crate::{ConversionError, UiMovie, UnknownEntries};

/// The deepest chain of movies importing from movies that import in turn.
pub const MAX_IMPORT_DEPTH: usize = 4;

/// The most movies one render reads to resolve imports.
pub const MAX_IMPORTED_MOVIES: usize = 16;

/// The most fonts one render takes from other movies.
pub const MAX_IMPORTED_FONTS: usize = 256;

/// The most bytes of movies one render reads to resolve imports, each counted by the larger of its
/// Entry size and the uncompressed length its header declares, checked before it is decompressed.
pub const MAX_IMPORT_BYTES: u64 = 256 << 20;

/// Why an import of a font could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportIssue {
    /// No Entry of the Archive has the file name of the URL.
    #[error("no Entry of the Archive matches the URL")]
    NotFound,
    /// The movie cannot be read or parsed.
    #[error("the movie cannot be read: {0}")]
    Unreadable(String),
    /// The movie does not export the name as a font.
    #[error("the movie does not export the name as a font")]
    NoSuchExport,
    /// Movies import from each other deeper than [`MAX_IMPORT_DEPTH`], or in a cycle.
    #[error("movies import from each other in a cycle or deeper than {MAX_IMPORT_DEPTH} levels")]
    TooDeep,
    /// The render would read more than [`MAX_IMPORTED_MOVIES`] movies, [`MAX_IMPORT_BYTES`] bytes
    /// or [`MAX_IMPORTED_FONTS`] fonts.
    #[error("the import budget of the render is used up")]
    OverBudget,
}

impl ImportIssue {
    /// Returns the stable `snake_case` code of the issue.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Unreadable(_) => "unreadable",
            Self::NoSuchExport => "no_such_export",
            Self::TooDeep => "too_deep",
            Self::OverBudget => "over_budget",
        }
    }
}

/// A font a movie imports that could not be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedImport {
    /// The URL of the exporting movie as the importing movie states it.
    pub url: String,
    /// The name imported.
    pub name: String,
    /// Why.
    pub issue: ImportIssue,
}

/// The movies read and the fonts not found while resolving the imports of a render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// The Entry paths of the movies read, in the order they were first needed.
    pub movies: Vec<String>,
    /// The fonts that could not be resolved.
    pub unresolved: Vec<UnresolvedImport>,
}

impl UiMovie<'_> {
    /// Opens the movie that the import URL `url` of this movie names: in an Archive the best match
    /// of the URL relative to this movie, with `.swf` and `.gfx` as alternatives and the folders
    /// of the UI and the English localisation as fallbacks; for a movie on disk the file relative
    /// to it.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry matches or the URL of a movie on
    /// disk is rooted, has a drive prefix or climbs above the folder of the movie,
    /// [`ConversionError::NoSuchFile`] when the file does not exist, and the
    /// errors of [`UiMovie::from_archive`] and [`UiMovie::from_file`] otherwise.
    pub fn import_source(&self, url: &str) -> Result<Self, ConversionError> {
        match self.origin() {
            Origin::Entry(archive) => {
                let language = self.font_language();
                let entry = candidates(archive, url, folder_of(self.name()), language)
                    .next()
                    .and_then(|at| archive.entries().get(at))
                    .ok_or_else(|| UnknownEntries(vec![url.to_owned()]))?;
                UiMovie::from_archive(archive, entry.path())
            }
            Origin::File(path) => {
                let relative = contained(url)
                    .ok_or_else(|| ConversionError::from(UnknownEntries(vec![url.to_owned()])))?;
                let folder = path.parent().unwrap_or_else(|| Path::new(""));
                UiMovie::from_file(&folder.join(relative))
            }
        }
    }
}
