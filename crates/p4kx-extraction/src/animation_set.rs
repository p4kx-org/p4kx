// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_model::Animation;
use rayon::prelude::*;

use crate::animation_load::{kind, load};
use crate::animation_select::{Gathered, read};
use crate::animation_sources::{ArchiveAnimationRead, disk_sources, open_entry, with_extras};
use crate::{CancelToken, ChunkSource, ConversionError, Entry, PathIndex};

/// The largest `.caf` or `.dba` file read into memory, far above the largest of a real Archive.
pub const MAX_ANIMATION_FILE_LEN: u64 = 256 << 20;

/// Why an additive animation is not written unless it is named.
pub(crate) const ADDITIVE_REASON: &str = "additive animation — needs a base animation";

/// Which animations of the sources to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationChoice {
    /// Every animation of every source.
    All,
    /// The animations the entries name: a name, the stem or file name of an animation or the end
    /// of its path, ignoring case; or the path of a `.caf` or `.dba` Entry or file, which
    /// selects all of its animations.
    Named(Vec<String>),
}

/// The most animation files and the most bytes of them that [`AnimationSet::from_archive_bounded`]
/// reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationBounds {
    /// The most `.caf` and `.dba` files.
    pub files: usize,
    /// The most uncompressed bytes of all of them.
    pub bytes: u64,
}

/// An animation file or an animation of a database that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnreadAnimation {
    /// The Entry path or file of the animation file.
    pub source: String,
    /// The animation's name when the file is a database and only this animation failed.
    pub name: Option<String>,
    /// Why it could not be read.
    pub reason: String,
}

/// The animations read for a model from the `.caf` files and `.dba` databases next to it, the
/// ones at the same place below `Data\Animations`, and the sources the caller names.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct AnimationSet {
    /// The animations that were chosen and read, ordered by source and then as in the source.
    pub animations: Vec<Animation>,
    /// The Entry path or file each of `animations` was read from, in step with it.
    pub origins: Vec<String>,
    /// The files on disk the animations were read from; empty for an Archive.
    pub inputs: Vec<PathBuf>,
    /// The files and animations that could not be read; a source named by the caller is an
    /// error instead.
    pub unread: Vec<UnreadAnimation>,
    /// The number of animation files read.
    pub files: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub(crate) label: String,
    pub(crate) explicit: bool,
    pub(crate) whole: bool,
}

pub(crate) fn normalised(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

pub(crate) fn stem(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

/// Returns whether `spec` names `animation`: its stem or file name, or the end of its path.
pub(crate) fn names(spec: &str, name: &str, path: &str) -> bool {
    let spec = normalised(spec);
    let path = normalised(path);
    spec == normalised(name)
        || path == spec
        || path
            .strip_suffix(&spec)
            .is_some_and(|before| before.ends_with('/'))
}

/// Returns the folders below which the files of the model at `entry` are looked for.
fn folders(entry: &str) -> Vec<String> {
    let path = normalised(entry);
    let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
    let mut folders = vec![folder.to_owned()];
    if let Some(rest) = folder.strip_prefix("data/")
        && !rest.starts_with("animations")
    {
        folders.push(format!("data/animations/{rest}"));
    }
    folders
}

fn same_folder(folder: &str, wanted: &str) -> bool {
    same_bytes(folder.as_bytes(), wanted)
}

fn same_bytes(found: &[u8], wanted: &str) -> bool {
    found.len() == wanted.len()
        && found.iter().zip(wanted.bytes()).all(|(found, wanted)| {
            let found = if *found == b'\\' { b'/' } else { *found };
            found.eq_ignore_ascii_case(&wanted)
        })
}

fn under_prefixes(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        path.as_bytes()
            .get(..prefix.len())
            .is_some_and(|start| same_bytes(start, prefix))
    })
}

fn in_folders(path: &str, folders: &[String]) -> bool {
    path.rfind(['/', '\\'])
        .and_then(|at| path.get(..at))
        .is_some_and(|folder| folders.iter().any(|wanted| same_folder(folder, wanted)))
}

pub(crate) fn folder_entries<'i>(index: &PathIndex<'i>, model_entry: &str) -> Vec<&'i Entry> {
    let folders = folders(model_entry);
    index
        .archive()
        .entries()
        .par_iter()
        .filter(|entry| {
            let path = entry.path();
            kind(path).is_some() && in_folders(path, &folders)
        })
        .collect()
}

fn within(bounds: &AnimationBounds, mut sizes: impl ExactSizeIterator<Item = u64>) -> bool {
    sizes.len() <= bounds.files
        && sizes
            .try_fold(0u64, u64::checked_add)
            .is_some_and(|bytes| bytes <= bounds.bytes)
}

pub(crate) fn source_of(entry: &Entry) -> Source {
    Source {
        label: entry.path().to_owned(),
        explicit: false,
        whole: false,
    }
}

impl AnimationSet {
    /// Reads the animations `choice` selects from the `.caf` and `.dba` Entries in the folder of
    /// the model Entry `model_entry`, in the folder at the same place below `Data\Animations`
    /// and the Entries `extra`, all looked up through `index`.
    ///
    /// A `.caf` file whose stem no name matches is not read, so a folder of thousands of
    /// animations costs little when a few are asked for. An additive animation is left out and
    /// listed in `unread` unless `choice` names it, or it is a `.caf` Entry of `extra`.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when an Entry of `extra` does not exist,
    /// [`ConversionError::NotAnimationFile`] when it is no `.caf` or `.dba` Entry,
    /// [`ConversionError::Animation`] when an Entry of `extra` or one `choice` names by path is no
    /// readable animation file, [`ConversionError::UnknownAnimations`] when a name of `choice`
    /// matches no animation, and another [`ConversionError`] when an Entry cannot be read.
    pub fn from_archive(
        index: &PathIndex<'_>,
        model_entry: &str,
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<Self, ConversionError> {
        Self::from_archive_cancellable(index, model_entry, choice, extra, &CancelToken::new())
    }

    /// As [`Self::from_archive`], stopping between files once `cancel` was asked to.
    ///
    /// # Errors
    ///
    /// As [`Self::from_archive`], and [`ConversionError::Cancelled`] when `cancel` stopped the
    /// read.
    pub fn from_archive_cancellable(
        index: &PathIndex<'_>,
        model_entry: &str,
        choice: &AnimationChoice,
        extra: &[String],
        cancel: &CancelToken,
    ) -> Result<Self, ConversionError> {
        let sources = folder_entries(index, model_entry)
            .into_iter()
            .map(source_of)
            .collect();
        Self::from_sources(index, sources, choice, extra, Some(cancel))
    }

    /// Reads every animation of the `.caf` and `.dba` Entries in the folders
    /// [`Self::from_archive`] looks in, unless they are more files or bytes than `bounds` allow;
    /// returns `None` then, before any of them is read.
    ///
    /// The read stops between files once `cancel` was asked to.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Cancelled`] when `cancel` stopped the read, and another
    /// [`ConversionError`] when an Entry cannot be read.
    pub fn from_archive_bounded(
        index: &PathIndex<'_>,
        model_entry: &str,
        bounds: AnimationBounds,
        cancel: &CancelToken,
    ) -> Result<Option<Self>, ConversionError> {
        let entries = folder_entries(index, model_entry);
        if !within(
            &bounds,
            entries.iter().map(|entry| entry.uncompressed_size()),
        ) {
            return Ok(None);
        }
        let sources = entries.into_iter().map(source_of).collect();
        Self::from_sources(index, sources, &AnimationChoice::All, &[], Some(cancel)).map(Some)
    }

    /// Reads the animations `choice` selects from the `.caf` and `.dba` Entries anywhere below
    /// the folders `folders` (Entry paths with `/` or `\`, ignoring case) and the Entries
    /// `extra`; see [`Self::from_archive`] for the rules of `choice` and the errors.
    ///
    /// This is how the animations of a Character are found: its skeleton's parameters list the
    /// folders of its body and its head.
    ///
    /// # Errors
    ///
    /// As [`Self::from_archive`].
    pub fn from_folders(
        index: &PathIndex<'_>,
        folders: &[&str],
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<Self, ConversionError> {
        Self::from_folders_with_archive_sources(index, folders, choice, extra)
            .map(ArchiveAnimationRead::into_set)
    }

    /// Reads selected animations and retains the Archive Entry bound to each successful read.
    ///
    /// # Errors
    /// As [`Self::from_folders`].
    pub fn from_folders_with_archive_sources(
        index: &PathIndex<'_>,
        folders: &[&str],
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<ArchiveAnimationRead, ConversionError> {
        let prefixes: Vec<String> = folders
            .iter()
            .map(|folder| format!("{}/", normalised(folder).trim_end_matches('/')))
            .collect();
        let sources: Vec<Source> = index
            .archive()
            .entries()
            .par_iter()
            .filter(|entry| kind(entry.path()).is_some() && under_prefixes(entry.path(), &prefixes))
            .map(source_of)
            .collect();
        Self::gather_sources(index, sources, choice, extra, None).map(ArchiveAnimationRead::from)
    }

    fn from_sources(
        index: &PathIndex<'_>,
        sources: Vec<Source>,
        choice: &AnimationChoice,
        extra: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ConversionError> {
        Self::gather_sources(index, sources, choice, extra, cancel).map(Self::from)
    }

    fn gather_sources(
        index: &PathIndex<'_>,
        sources: Vec<Source>,
        choice: &AnimationChoice,
        extra: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Gathered<Animation>, ConversionError> {
        let sources = with_extras(index, sources, choice, extra)?;
        read(
            sources,
            choice,
            |label| open_entry(index, label, cancel),
            |file, label, kind, _| load(file, label, kind),
        )
    }

    /// Reads the animations `choice` selects from the `.caf` and `.dba` files in the folder of
    /// the model file `model` and the files `extra`; see [`Self::from_archive`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_archive`], with [`ConversionError::NoSuchFile`] for a file that does not
    /// exist.
    pub fn from_disk(
        model: &Path,
        choice: &AnimationChoice,
        extra: &[PathBuf],
    ) -> Result<Self, ConversionError> {
        let sources = disk_sources(model, choice, extra)?;
        let mut set = Self::from(read(
            sources,
            choice,
            |label| ChunkSource::from_file(Path::new(label)),
            |file, label, kind, _| load(file, label, kind),
        )?);
        set.inputs = set.origins.iter().map(PathBuf::from).collect();
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimationBounds, within};

    const BOUNDS: AnimationBounds = AnimationBounds {
        files: 2,
        bytes: 10,
    };

    #[test]
    fn sizes_within_both_bounds_pass() {
        assert!(within(&BOUNDS, [4, 6].into_iter()), "at both bounds");
    }

    #[test]
    fn too_many_files_or_bytes_fail() {
        assert!(!within(&BOUNDS, [1, 1, 1].into_iter()), "three files");
        assert!(!within(&BOUNDS, [5, 6].into_iter()), "eleven bytes");
    }

    #[test]
    fn a_sum_that_overflows_fails_instead_of_wrapping() {
        let huge = AnimationBounds {
            files: 2,
            bytes: u64::MAX,
        };
        assert!(!within(&huge, [u64::MAX, 1].into_iter()), "overflow");
    }
}
