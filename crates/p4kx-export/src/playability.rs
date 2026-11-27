// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

/// Who an item, ship or ground vehicle is for, as the Records structurally say.
///
/// Catalog rows write the name of the value in `playability.value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Playability {
    /// Has the structural markers of a player entity: a hangar-ready ship without AI crew or hull,
    /// or an item with a shop display name. Not proof that a player can obtain it.
    Player,
    /// Part of the AI: a ship with an NPC crew, an AI hull setup or a spawn-only configuration.
    Ai,
    /// Part of a mission: an unmanned mission or salvage ship, or an item of a mission folder.
    Mission,
    /// Neither: a ship whose `inclusionMode` is `DoNotInclude`, or an item without a
    /// player-visible name.
    Special,
}

impl Playability {
    /// Every value, in the order the summary lists them.
    pub const ALL: [Self; 4] = [Self::Player, Self::Ai, Self::Mission, Self::Special];

    /// Returns the name, as the catalog writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Ai => "ai",
            Self::Mission => "mission",
            Self::Special => "special",
        }
    }
}

/// How many entities of one kind carry each [`Playability`], and how many carry none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayabilityCounts {
    player: usize,
    ai: usize,
    mission: usize,
    special: usize,
    unclassified: usize,
}

impl PlayabilityCounts {
    /// Counts one entity with `playability`.
    pub(crate) const fn count(&mut self, playability: Option<Playability>) {
        let counter = match playability {
            Some(Playability::Player) => &mut self.player,
            Some(Playability::Ai) => &mut self.ai,
            Some(Playability::Mission) => &mut self.mission,
            Some(Playability::Special) => &mut self.special,
            None => &mut self.unclassified,
        };
        *counter = counter.saturating_add(1);
    }

    /// Returns the entities that carry `playability`.
    #[must_use]
    pub const fn of(&self, playability: Playability) -> usize {
        match playability {
            Playability::Player => self.player,
            Playability::Ai => self.ai,
            Playability::Mission => self.mission,
            Playability::Special => self.special,
        }
    }

    /// Returns the number of entities whose playability value is `null`.
    #[must_use]
    pub const fn unclassified(&self) -> usize {
        self.unclassified
    }
}

/// The playability counts of the items and of the ships.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayabilitySummary {
    /// The items.
    pub items: PlayabilityCounts,
    /// The ships and ground vehicles.
    pub ships: PlayabilityCounts,
}
