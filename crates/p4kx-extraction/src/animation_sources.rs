// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::{Path, PathBuf};

use p4kx_archive::EntryOrigin;
use p4kx_model::{
    Animation, AnimationChunkCoordinates, AnimationClipCoordinates, AnimationDatabaseSelection,
    PositionedAnimation,
};

use crate::animation_load::{Loaded, kind, load_with_positions};
use crate::animation_select::{self, Gathered, MatchEvidence};
use crate::animation_set::{MAX_ANIMATION_FILE_LEN, Source, folder_entries, source_of};
use crate::{AnimationChoice, AnimationSet, CancelToken, ChunkSource, ConversionError, PathIndex};

#[path = "archive_animation_read.rs"]
mod read;

pub use read::{ArchiveAnimationRead, ArchiveClipEvidence};

/// The selected decoded clip and the physical Archive Entry that supplied it.
#[derive(Debug)]
pub struct ArchivePositionedAnimation {
    animation: PositionedAnimation,
    label: String,
    origin: Option<EntryOrigin>,
}

impl ArchivePositionedAnimation {
    /// Reads the first non-additive named clip from the explicit sources in normal selection order.
    ///
    /// # Errors
    /// As [`AnimationSet::from_folders`] for explicit sources and a named selection.
    pub fn read_first_non_additive(
        index: &PathIndex<'_>,
        name: &str,
        extra: &[String],
    ) -> Result<Option<Self>, ConversionError> {
        let choice = AnimationChoice::Named(vec![name.to_owned()]);
        let sources = with_extras(index, Vec::new(), &choice, extra)?;
        let gathered = animation_select::read(
            sources,
            &choice,
            |label| open_entry(index, label, None),
            |source, label, kind, _| load_with_positions(source, label, kind),
        )?;
        Ok(Self::gathered(gathered).find(|clip| !clip.animation().animation().additive))
    }

    fn gathered(gathered: Gathered<PositionedAnimation>) -> impl Iterator<Item = Self> {
        gathered
            .animations
            .into_iter()
            .zip(gathered.origins)
            .zip(gathered.archive_sources)
            .map(|((animation, label), origin)| Self {
                animation,
                label,
                origin,
            })
    }

    /// Returns the decoded clip and its original ordinal coordinates.
    #[must_use]
    pub const fn animation(&self) -> &PositionedAnimation {
        &self.animation
    }

    /// Returns the path chosen by the animation selection.
    #[must_use]
    pub fn source_name(&self) -> &str {
        &self.label
    }

    /// Returns the physical Archive Entry admitted by the read.
    #[must_use]
    pub const fn origin(&self) -> Option<&EntryOrigin> {
        self.origin.as_ref()
    }
}

/// The decoded model animations and the physical inputs that determined admission.
#[derive(Debug)]
pub struct ArchiveModelAnimations {
    clips: Vec<ArchivePositionedAnimation>,
    admissions: Vec<ArchiveAnimationAdmission>,
}

impl ArchiveModelAnimations {
    /// Reads the model's normal animation selection with the original admission selectors.
    ///
    /// # Errors
    /// As [`AnimationSet::from_archive`] for [`AnimationChoice::All`].
    pub fn read(index: &PathIndex<'_>, model_entry: &str) -> Result<Self, ConversionError> {
        let sources = folder_entries(index, model_entry)
            .into_iter()
            .map(source_of)
            .collect();
        let mut admissions = Vec::new();
        let gathered = animation_select::read_observing(
            sources,
            &AnimationChoice::All,
            |label| open_entry(index, label, None),
            |source, label, kind, _| load_with_positions(source, label, kind),
            MatchEvidence::Off,
            |origin, loaded| {
                let authored_empty = loaded.database.is_some()
                    && loaded.animations.is_empty()
                    && loaded.unread.is_empty();
                if authored_empty || !loaded.animations.is_empty() {
                    admissions.push(ArchiveAnimationAdmission::of(origin, loaded));
                }
            },
        )?;
        Ok(Self {
            clips: ArchivePositionedAnimation::gathered(gathered).collect(),
            admissions,
        })
    }

    /// Returns the clips admitted to the output, in normal order.
    #[must_use]
    pub fn clips(&self) -> &[ArchivePositionedAnimation] {
        &self.clips
    }

    /// Returns the successfully read inputs whose metadata determined the clip count.
    #[must_use]
    pub fn admissions(&self) -> &[ArchiveAnimationAdmission] {
        &self.admissions
    }
}

/// Original admission metadata retained for one successfully read animation Entry.
#[derive(Debug)]
#[non_exhaustive]
pub struct ArchiveAnimationAdmission {
    /// The physical Archive Entry that supplied the selection.
    pub origin: Option<EntryOrigin>,
    /// The metadata chunk actually selected by the reader.
    pub metadata: Option<AnimationChunkCoordinates>,
    /// The controller chunk actually selected by the reader.
    pub data: Option<AnimationChunkCoordinates>,
    /// The authored DBA count word, including an empty database.
    pub declared_count: Option<Range<u64>>,
    /// Original flag words evaluated for decoded clips, including additive rejections.
    pub flags: Vec<AnimationFlagRun>,
    /// Whether all evaluated flags were observed with their original coordinates.
    pub complete: bool,
}

impl ArchiveAnimationAdmission {
    fn of(origin: Option<&EntryOrigin>, loaded: &Loaded<PositionedAnimation>) -> Self {
        let database = loaded.database.as_ref();
        let first_clip = loaded
            .animations
            .first()
            .and_then(PositionedAnimation::coordinates);
        let metadata = database
            .map(AnimationDatabaseSelection::metadata_chunk)
            .or_else(|| first_clip.map(AnimationClipCoordinates::metadata_chunk));
        let data = database
            .map(AnimationDatabaseSelection::data_chunk)
            .or_else(|| first_clip.map(AnimationClipCoordinates::data_chunk));
        let mut flags: Vec<AnimationFlagRun> = Vec::new();
        let mut complete = metadata.is_some() && data.is_some();
        for clip in &loaded.animations {
            if let Some(coords) = clip.coordinates() {
                let ordinal = clip.clip_ordinal() as u64;
                let bytes = coords.flags();
                if !flags
                    .last_mut()
                    .is_some_and(|run| run.extend(ordinal, &bytes))
                {
                    flags.push(AnimationFlagRun {
                        first_clip: ordinal,
                        stride: bytes.end - bytes.start,
                        first_word: bytes,
                        count: 1,
                    });
                }
            } else {
                complete = false;
            }
        }
        Self {
            origin: origin.cloned(),
            metadata: metadata.cloned(),
            data: data.cloned(),
            declared_count: database.map(AnimationDatabaseSelection::count_range),
            flags,
            complete,
        }
    }
}

/// A strided sequence of original flag words from consecutive clip ordinals.
#[derive(Debug)]
#[non_exhaustive]
pub struct AnimationFlagRun {
    /// The original clip ordinal of the first word.
    pub first_clip: u64,
    /// The half-open interval of the first flag word.
    pub first_word: Range<u64>,
    /// The byte stride between flag words.
    pub stride: u64,
    /// The number of consecutive clip ordinals.
    pub count: u64,
}

impl AnimationFlagRun {
    fn extend(&mut self, clip: u64, bytes: &Range<u64>) -> bool {
        if clip != self.first_clip + self.count {
            return false;
        }
        if self.count == 1 {
            let Some(stride) = bytes.start.checked_sub(self.first_word.start) else {
                return false;
            };
            self.stride = stride;
        }
        let shift = self.count * self.stride;
        if *bytes != (self.first_word.start + shift..self.first_word.end + shift) {
            return false;
        }
        self.count += 1;
        true
    }
}

pub(crate) fn open_entry(
    index: &PathIndex<'_>,
    label: &str,
    cancel: Option<&CancelToken>,
) -> Result<ChunkSource, ConversionError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(ConversionError::Cancelled);
    }
    ChunkSource::from_index_limited(index, label, MAX_ANIMATION_FILE_LEN)
}

impl From<Gathered<Animation>> for AnimationSet {
    fn from(gathered: Gathered<Animation>) -> Self {
        Self {
            animations: gathered.animations,
            origins: gathered.origins,
            inputs: Vec::new(),
            unread: gathered.unread,
            files: gathered.files,
        }
    }
}

/// Adds the Entries `extra` and the `.caf` and `.dba` Entries `choice` names by path to
/// `sources`.
pub(crate) fn with_extras(
    index: &PathIndex<'_>,
    mut sources: Vec<Source>,
    choice: &AnimationChoice,
    extra: &[String],
) -> Result<Vec<Source>, ConversionError> {
    for path in extra {
        let entry = index
            .find(path)
            .ok_or_else(|| crate::UnknownEntries(vec![path.clone()]))?;
        if kind(entry.path()).is_none() {
            return Err(ConversionError::NotAnimationFile {
                file: entry.path().to_owned(),
            });
        }
        sources.push(Source {
            label: entry.path().to_owned(),
            explicit: true,
            whole: false,
        });
    }
    let named = match choice {
        AnimationChoice::Named(specs) => specs.as_slice(),
        AnimationChoice::All => &[],
    };
    for spec in named {
        if kind(spec).is_some()
            && let Some(entry) = index.find(spec)
        {
            sources.push(Source {
                label: entry.path().to_owned(),
                explicit: true,
                whole: true,
            });
        }
    }
    Ok(sources)
}

/// Returns the `.caf` and `.dba` files next to the model file `model`, the files `extra` and the
/// files `choice` names by path.
pub(crate) fn disk_sources(
    model: &Path,
    choice: &AnimationChoice,
    extra: &[PathBuf],
) -> Result<Vec<Source>, ConversionError> {
    let folder = model
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty());
    let listing = std::fs::read_dir(folder.unwrap_or_else(|| Path::new(".")));
    let mut sources: Vec<Source> = listing
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && kind(&path.to_string_lossy()).is_some())
        .map(|path| Source {
            label: path.display().to_string(),
            explicit: false,
            whole: false,
        })
        .collect();
    for path in extra {
        let label = path.display().to_string();
        if kind(&label).is_none() {
            return Err(ConversionError::NotAnimationFile { file: label });
        }
        sources.push(Source {
            label,
            explicit: true,
            whole: false,
        });
    }
    if let AnimationChoice::Named(specs) = choice {
        sources.extend(
            specs
                .iter()
                .filter(|spec| kind(spec).is_some() && Path::new(spec).is_file())
                .map(|spec| Source {
                    label: spec.clone(),
                    explicit: true,
                    whole: true,
                }),
        );
    }
    Ok(sources)
}
