// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::EntryOrigin;
use p4kx_model::{Animation, AnimationClipCoordinates, PositionedAnimation};

use crate::AnimationSet;
use crate::animation_select::Gathered;

/// The selected animations and their bound Archive reads.
#[derive(Debug, Clone, Default)]
pub struct ArchiveAnimationRead {
    set: AnimationSet,
    archive_sources: Vec<Option<EntryOrigin>>,
    clip_evidence: Vec<ArchiveClipEvidence>,
    matched_specs: Vec<Vec<String>>,
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

    /// Returns the original position of one selected clip, when observed during its decode.
    #[must_use]
    pub fn clip_evidence(&self, index: usize) -> Option<&ArchiveClipEvidence> {
        self.clip_evidence.get(index)
    }

    /// Returns the selection specs that matched the decoded clip at this position.
    #[must_use]
    pub fn matched_specs(&self, index: usize) -> &[String] {
        self.matched_specs.get(index).map_or(&[], Vec::as_slice)
    }

    /// Returns the selected animations without the Archive evidence.
    #[must_use]
    pub fn into_set(self) -> AnimationSet {
        self.set
    }
}

/// Original clip coordinates retained by the same decode that selected the clip.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ArchiveClipEvidence {
    /// Original clip ordinal in its containing CAF or DBA.
    pub ordinal: usize,
    /// Original metadata and name positions, when decoded.
    pub coordinates: Option<AnimationClipCoordinates>,
}

impl From<Gathered<PositionedAnimation>> for ArchiveAnimationRead {
    fn from(gathered: Gathered<PositionedAnimation>) -> Self {
        let mut clips = Vec::with_capacity(gathered.animations.len());
        let mut clip_evidence = Vec::with_capacity(gathered.animations.len());
        for clip in gathered.animations {
            clip_evidence.push(ArchiveClipEvidence {
                ordinal: clip.clip_ordinal(),
                coordinates: clip.coordinates().cloned(),
            });
            clips.push(clip.into_animation());
        }
        Self {
            set: AnimationSet {
                animations: clips,
                origins: gathered.origins,
                inputs: Vec::new(),
                unread: gathered.unread,
                files: gathered.files,
            },
            archive_sources: gathered.archive_sources,
            clip_evidence,
            matched_specs: gathered.matched_specs,
        }
    }
}

impl From<Gathered<Animation>> for ArchiveAnimationRead {
    fn from(mut gathered: Gathered<Animation>) -> Self {
        let archive_sources = std::mem::take(&mut gathered.archive_sources);
        Self {
            set: gathered.into(),
            archive_sources,
            clip_evidence: Vec::new(),
            matched_specs: Vec::new(),
        }
    }
}
