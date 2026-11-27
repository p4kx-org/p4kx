// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourcedValue};

use super::{Playback, TagEvidence, TagMatch, WeaponPlayerAnimationCandidate};
use crate::catalog_read::original::PhysicalEntryRef;

impl WeaponPlayerAnimationCandidate {
    /// Borrows the original physical weapon Record position.
    #[must_use]
    pub const fn weapon_record_index(&self) -> &SourcedValue<u32> {
        &self.weapon_record_index
    }

    /// Borrows the original weapon `AttachDef` component position.
    #[must_use]
    pub const fn attachment_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.attachment_component_ordinal
    }

    /// Borrows the player actor Record position.
    #[must_use]
    pub const fn player_record_index(&self) -> &SourcedValue<u32> {
        &self.player_record_index
    }

    /// Borrows the player actor component position.
    #[must_use]
    pub const fn player_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.player_component_ordinal
    }

    /// Borrows the physical parent database Entry.
    #[must_use]
    pub const fn branch_database(&self) -> &PhysicalEntryRef {
        &self.branch_database
    }

    /// Borrows the authored branch position in the parent database.
    #[must_use]
    pub const fn branch_position(&self) -> &SourcedValue<SourceLocation> {
        &self.branch_position
    }

    /// Borrows the physical option database Entry.
    #[must_use]
    pub const fn option_database(&self) -> &PhysicalEntryRef {
        &self.option_database
    }

    /// Borrows the authored option position in its database.
    #[must_use]
    pub const fn option_position(&self) -> &SourcedValue<SourceLocation> {
        &self.option_position
    }

    /// Borrows the shared tag declaration evidence.
    #[must_use]
    pub const fn tag_evidence(&self) -> &SourcedValue<TagEvidence> {
        &self.tag_evidence
    }

    /// Returns the tool-owned playback verification status.
    #[must_use]
    pub const fn playback(&self) -> Playback {
        self.playback
    }

    /// Borrows the original token and declaration matches in order.
    #[must_use]
    pub fn matches(&self) -> &[TagMatch] {
        &self.matches
    }
}

impl TagMatch {
    /// Borrows the weapon tag token position.
    #[must_use]
    pub const fn weapon_token(&self) -> &SourcedValue<usize> {
        &self.weapon_token
    }

    /// Borrows the option tag token position.
    #[must_use]
    pub const fn option_token(&self) -> &SourcedValue<usize> {
        &self.option_token
    }

    /// Borrows the physical Entry that owns the original declaration.
    #[must_use]
    pub const fn source(&self) -> &PhysicalEntryRef {
        &self.source
    }

    /// Borrows the original tag declaration position.
    #[must_use]
    pub const fn declaration(&self) -> &SourcedValue<SourceLocation> {
        &self.declaration
    }
}
