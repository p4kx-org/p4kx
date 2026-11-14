// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Physical CAF and DBA headers independent of rendering choices.
#[path = "animation_catalogue_headers.rs"]
pub mod headers;

use std::path::{Path, PathBuf};

use p4kx_model::AnimationHeader;

use crate::animation_load::load_listed;
use crate::animation_select::{Gathered, read};
use crate::animation_set::{folder_entries, source_of};
use crate::animation_sources::{disk_sources, open_entry, with_extras};
use crate::{AnimationChoice, ChunkSource, ConversionError, PathIndex, UnreadAnimation};

/// The animations found for a model, described by their headers: names, durations, frame rates,
/// the additive flag, tracks and key counts, with no key decoded.
///
/// It finds the files [`crate::AnimationSet::from_archive`] reads for [`AnimationChoice::All`] and
/// lists the same additive animations as unread; it does not notice a key format the reader
/// does not know.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct AnimationListing {
    /// The animations, ordered by source and then as in the source.
    pub animations: Vec<AnimationHeader>,
    /// The Entry path or file each of `animations` was read from, in step with it.
    pub origins: Vec<String>,
    /// The files and animations that could not be read, and the additive ones.
    pub unread: Vec<UnreadAnimation>,
    /// The number of animation files read.
    pub files: usize,
}

impl From<Gathered<AnimationHeader>> for AnimationListing {
    fn from(gathered: Gathered<AnimationHeader>) -> Self {
        Self {
            animations: gathered.animations,
            origins: gathered.origins,
            unread: gathered.unread,
            files: gathered.files,
        }
    }
}

impl AnimationListing {
    /// Lists the animations of the `.caf` and `.dba` Entries in the folder of the model Entry
    /// `model_entry`, in the folder at the same place below `Data\Animations` and the Entries
    /// `extra`, all looked up through `index`.
    ///
    /// # Errors
    ///
    /// As [`crate::AnimationSet::from_archive`] for [`AnimationChoice::All`].
    pub fn from_archive(
        index: &PathIndex<'_>,
        model_entry: &str,
        extra: &[String],
    ) -> Result<Self, ConversionError> {
        let sources = folder_entries(index, model_entry)
            .into_iter()
            .map(source_of)
            .collect();
        let choice = AnimationChoice::All;
        let sources = with_extras(index, sources, &choice, extra)?;
        read(
            sources,
            &choice,
            |label| open_entry(index, label, None),
            load_listed,
        )
        .map(Self::from)
    }

    /// Lists the animations of the `.caf` and `.dba` files in the folder of the model file `model`
    /// and the files `extra`.
    ///
    /// # Errors
    ///
    /// As [`crate::AnimationSet::from_disk`] for [`AnimationChoice::All`].
    pub fn from_disk(model: &Path, extra: &[PathBuf]) -> Result<Self, ConversionError> {
        let choice = AnimationChoice::All;
        let sources = disk_sources(model, &choice, extra)?;
        read(
            sources,
            &choice,
            |label| ChunkSource::from_file(Path::new(label)),
            load_listed,
        )
        .map(Self::from)
    }
}
