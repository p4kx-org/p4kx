// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Physical Wwise audio placement within an Archive directory.

/// Canonical Archive path prefix for Wwise sources.
pub const SOUND_FOLDER: &str = "Data\\Sounds\\wwise\\";

const MEDIA_FOLDER: &str = "Media";

/// Language placement observed in a recognized Wwise directory layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Folder<'p> {
    /// A recognized source location without a language directory.
    Neutral,
    /// The original, case-preserving language directory name.
    Language(&'p str),
}

impl<'p> Folder<'p> {
    /// Returns the original language name, when the location has one.
    #[must_use]
    pub const fn language(self) -> Option<&'p str> {
        match self {
            Self::Neutral => None,
            Self::Language(language) => Some(language),
        }
    }
}

/// Returns the direct language folder of a bank in a directory named Wwise.
#[must_use]
pub fn bank_language(path: &str) -> Option<&str> {
    let mut folders = path.rsplit(['\\', '/']).skip(1);
    let folder = folders.next()?;
    let parent = folders.next()?;
    (parent.eq_ignore_ascii_case("wwise") && !folder.is_empty()).then_some(folder)
}

/// Returns the observed placement of a BNK in the canonical Wwise directory.
#[must_use]
pub fn bank_folder(path: &str) -> Option<Folder<'_>> {
    let (folders, _) = wwise_file(path, "bnk")?;
    if folders.is_empty() {
        return Some(Folder::Neutral);
    }
    (!folders.contains(['\\', '/'])).then_some(Folder::Language(folders))
}

/// Returns the observed placement of a WEM in the canonical Wwise directory.
#[must_use]
pub fn loose_folder(path: &str) -> Option<Folder<'_>> {
    loose_file(path).map(|(_, folder)| folder)
}

/// The short ID and language placement named by a numeric loose WEM path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LooseName<'p> {
    /// Numeric stem of the original file name.
    pub id: u32,
    /// Language folder, when the path has one.
    pub language: Option<&'p str>,
}

/// Returns the numeric short ID and observed language of a canonical loose WEM path.
#[must_use]
pub fn loose_name(path: &str) -> Option<LooseName<'_>> {
    let (stem, folder) = loose_file(path)?;
    if !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(LooseName {
        id: stem.parse().ok()?,
        language: folder.language(),
    })
}

fn loose_file(path: &str) -> Option<(&str, Folder<'_>)> {
    let (folders, stem) = wwise_file(path, "wem")?;
    let mut language = None;
    if !folders.is_empty() {
        for folder in folders.split(['\\', '/']) {
            if folder.eq_ignore_ascii_case(MEDIA_FOLDER) {
                continue;
            }
            if folder.is_empty() || language.replace(folder).is_some() {
                return None;
            }
        }
    }
    Some((stem, language.map_or(Folder::Neutral, Folder::Language)))
}

fn wwise_file<'p>(path: &'p str, extension: &str) -> Option<(&'p str, &'p str)> {
    let (prefix, rest) = path.split_at_checked(SOUND_FOLDER.len())?;
    if !prefix.bytes().zip(SOUND_FOLDER.bytes()).all(|(a, b)| {
        let a = if a == b'/' { b'\\' } else { a };
        a.eq_ignore_ascii_case(&b)
    }) {
        return None;
    }
    let (folders, file) = rest.rsplit_once(['\\', '/']).unwrap_or(("", rest));
    let (stem, actual) = file.rsplit_once('.')?;
    (actual.eq_ignore_ascii_case(extension) && !stem.is_empty()).then_some((folders, stem))
}
