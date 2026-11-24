// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::UnreadClipCoordinates;

use crate::UnreadAnimation;

/// An unread model animation with source positions retained by its decoder.
#[derive(Debug)]
pub struct PositionedUnreadAnimation {
    value: UnreadAnimation,
    coordinates: Option<UnreadClipCoordinates>,
}

impl PositionedUnreadAnimation {
    pub(crate) const fn new(
        value: UnreadAnimation,
        coordinates: Option<UnreadClipCoordinates>,
    ) -> Self {
        Self { value, coordinates }
    }

    pub(crate) fn into_unread(self) -> UnreadAnimation {
        self.value
    }
    /// Returns the unread row in its normal representation.
    #[must_use]
    pub const fn value(&self) -> &UnreadAnimation {
        &self.value
    }

    /// Returns the authored name and additive flag sites when observed.
    #[must_use]
    pub const fn coordinates(&self) -> Option<&UnreadClipCoordinates> {
        self.coordinates.as_ref()
    }
}
