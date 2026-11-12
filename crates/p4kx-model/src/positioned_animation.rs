// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Animation, SkippedAnimation};

/// A decoded clip paired with the source positions retained by that decode.
#[derive(Debug, Clone)]
pub struct PositionedAnimation {
    animation: Animation,
    clip_ordinal: usize,
    controller_ordinals: Vec<usize>,
}

impl PositionedAnimation {
    pub(crate) const fn new(
        animation: Animation,
        clip_ordinal: usize,
        controller_ordinals: Vec<usize>,
    ) -> Self {
        Self {
            animation,
            clip_ordinal,
            controller_ordinals,
        }
    }

    /// Returns the decoded clip.
    #[must_use]
    pub const fn animation(&self) -> &Animation {
        &self.animation
    }

    /// Returns the original zero-based clip ordinal in its file.
    #[must_use]
    pub const fn clip_ordinal(&self) -> usize {
        self.clip_ordinal
    }

    /// Returns the original controller ordinal of one decoded track.
    #[must_use]
    pub fn controller_ordinal(&self, track: usize) -> Option<usize> {
        self.controller_ordinals.get(track).copied()
    }
}

/// A DBA's admitted clips and the clips that could not be decoded.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PositionedDba {
    /// The decoded clips in source order with their original positions.
    pub animations: Vec<PositionedAnimation>,
    /// The original clips that could not be decoded.
    pub skipped: Vec<SkippedAnimation>,
}
