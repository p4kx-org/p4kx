// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::EntryOrigin;
use p4kx_model::UnreadClipCoordinates;

use crate::UnreadAnimation;

/// An unread model animation with source positions retained by its decoder.
#[derive(Debug)]
pub struct PositionedUnreadAnimation {
    value: UnreadAnimation,
    evidence: UnreadClipEvidence,
}

#[derive(Debug, Clone)]
pub(crate) struct UnreadClipEvidence {
    pub(crate) coordinates: Option<UnreadClipCoordinates>,
    pub(crate) origin: Option<EntryOrigin>,
}

impl PositionedUnreadAnimation {
    pub(crate) const fn new(
        value: UnreadAnimation,
        coordinates: Option<UnreadClipCoordinates>,
    ) -> Self {
        Self {
            value,
            evidence: UnreadClipEvidence {
                coordinates,
                origin: None,
            },
        }
    }

    pub(crate) fn into_unread(self) -> UnreadAnimation {
        self.value
    }
    pub(crate) fn with_origin(mut self, origin: Option<&EntryOrigin>) -> Self {
        self.evidence.origin = origin.cloned();
        self
    }

    pub(crate) fn into_parts(self) -> (UnreadAnimation, UnreadClipEvidence) {
        (self.value, self.evidence)
    }

    /// Returns the unread row in its normal representation.
    #[must_use]
    pub const fn value(&self) -> &UnreadAnimation {
        &self.value
    }

    /// Returns the authored name and additive flag sites when observed.
    #[must_use]
    pub const fn coordinates(&self) -> Option<&UnreadClipCoordinates> {
        self.evidence.coordinates.as_ref()
    }
}
