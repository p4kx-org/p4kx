// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_archive::EntryOrigin;
use p4kx_model::{Animation, AnimationClipCoordinates, PositionedAnimation};

use super::positioned_unread_animation::UnreadClipEvidence;
use super::{open_entry, with_extras};
use crate::animation_load::load_with_positions;
use crate::animation_select::{self, Gathered, ReadEvidence};
use crate::animation_set::{folder_entries, source_of};
use crate::{
    AnimationBounds, AnimationChoice, AnimationSet, CancelToken, ConversionError, FieldOrigin,
    InputScope, PathIndex, PositionedUnreadAnimation, SourceCoordinateOmission,
    SourceEvidenceError, SourceInput, SourceUseRole, SourceUses,
};

/// The selected animations and their bound Archive reads.
#[derive(Debug, Clone, Default)]
pub struct ArchiveAnimationRead {
    set: AnimationSet,
    archive_sources: Vec<Option<EntryOrigin>>,
    clip_evidence: Vec<ArchiveClipEvidence>,
    matched_specs: Vec<Vec<String>>,
    unread_evidence: Vec<UnreadClipEvidence>,
}

impl ArchiveAnimationRead {
    /// Reads the normal standalone model selection with clip and unread source evidence.
    ///
    /// # Errors
    /// Returns the same errors as [`AnimationSet::from_archive`].
    pub fn for_model(
        index: &PathIndex<'_>,
        model_entry: &str,
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<Self, ConversionError> {
        let sources = folder_entries(index, model_entry)
            .into_iter()
            .map(source_of)
            .collect();
        let sources = with_extras(index, sources, choice, extra)?;
        animation_select::read_observing(
            sources,
            choice,
            |label| open_entry(index, label, None),
            |file, label, kind, _| load_with_positions(file, label, kind),
            ReadEvidence::SelectedAndUnread,
            |_, _| {},
        )
        .map(Self::from)
    }

    /// Reads the bounded preview selection and retains each selected clip's Archive Entry.
    ///
    /// # Errors
    /// Returns the same errors as [`AnimationSet::from_archive_bounded`].
    pub fn for_model_bounded(
        index: &PathIndex<'_>,
        model_entry: &str,
        bounds: AnimationBounds,
        cancel: &CancelToken,
    ) -> Result<Option<Self>, ConversionError> {
        AnimationSet::gather_archive_bounded(index, model_entry, bounds, cancel)
            .map(|read| read.map(Self::from))
    }

    /// Captures incomplete count evidence from the original Entries of selected clips.
    ///
    /// # Errors
    /// Returns an error when a contributing Entry use cannot be registered.
    pub fn selected_count_origin(
        &self,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut seen = HashSet::new();
        let mut inputs = Vec::new();
        for entry in self.archive_sources.iter().flatten() {
            if seen.insert(entry.binding()) {
                let payload = uses.observe(SourceInput::Archive(entry), InputScope::Payload)?;
                inputs.push(payload.at_opaque(
                    SourceUseRole::Conversion,
                    SourceCoordinateOmission::Unavailable,
                )?);
            }
        }
        Ok(uses.origin(&inputs, false))
    }

    pub(crate) fn unread_evidence(&self, index: usize) -> Option<&UnreadClipEvidence> {
        self.unread_evidence.get(index)
    }

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
        let (unread, unread_evidence) = gathered
            .unread
            .into_iter()
            .map(PositionedUnreadAnimation::into_parts)
            .unzip();
        Self {
            set: AnimationSet {
                animations: clips,
                origins: gathered.origins,
                inputs: Vec::new(),
                unread,
                files: gathered.files,
            },
            archive_sources: gathered.archive_sources,
            clip_evidence,
            matched_specs: gathered.matched_specs,
            unread_evidence,
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
            unread_evidence: Vec::new(),
        }
    }
}
