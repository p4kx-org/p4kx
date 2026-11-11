// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_archive::EntryOrigin;
use p4kx_model::Animation;

use crate::animation_load::kind;
use crate::animation_select::Gathered;
use crate::animation_set::{MAX_ANIMATION_FILE_LEN, Source};
use crate::{AnimationChoice, AnimationSet, CancelToken, ChunkSource, ConversionError, PathIndex};

/// The selected animations and their bound Archive reads.
#[derive(Debug, Clone, Default)]
pub struct ArchiveAnimationRead {
    set: AnimationSet,
    archive_sources: Vec<Option<EntryOrigin>>,
}

impl ArchiveAnimationRead {
    /// Returns the selected animations in source order.
    #[must_use]
    pub const fn set(&self) -> &AnimationSet {
        &self.set
    }

    /// Returns the Archive Entry that supplied the selected animation at `index`.
    #[must_use]
    pub fn archive_source(&self, index: usize) -> Option<&EntryOrigin> {
        self.archive_sources.get(index)?.as_ref()
    }

    /// Returns the selected animations without the Archive evidence.
    #[must_use]
    pub fn into_set(self) -> AnimationSet {
        self.set
    }
}

impl From<Gathered<Animation>> for ArchiveAnimationRead {
    fn from(mut gathered: Gathered<Animation>) -> Self {
        let archive_sources = std::mem::take(&mut gathered.archive_sources);
        Self {
            set: gathered.into(),
            archive_sources,
        }
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
