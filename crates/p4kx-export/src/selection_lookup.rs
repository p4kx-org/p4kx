// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::selection::{Kind, KindRule};

impl Kind {
    /// Returns whether the kind is read by [`read_crafting`](crate::read_crafting) or by the
    /// contract reader rather than by the generic selection of the catalog build.
    pub(crate) const fn is_crafting(self) -> bool {
        matches!(
            self,
            Self::Blueprints
                | Self::GameplayProperties
                | Self::QualityDistributions
                | Self::QualityQuantizations
                | Self::BlueprintPools
                | Self::DismantleProcesses
                | Self::Contracts
                | Self::MissionLocationTemplates
                | Self::ContractTemplates
                | Self::MissionBrokerEntries
        )
    }

    /// Returns the rule that selects the kind's Records, `None` for a [derived](Self::is_derived)
    /// kind: [`Kind::Paints`] and [`Kind::Loadouts`] are made of the entities of other kinds and
    /// [`Kind::HarvestableLocations`] and [`Kind::Shops`] read Archive files, not Records.
    #[must_use]
    pub const fn rule(self) -> Option<KindRule> {
        if self.is_derived() {
            None
        } else {
            Some(self.records())
        }
    }

    /// Returns the first kind in [`Kind::ALL`] one of whose prefixes `record_path` starts with,
    /// and never a derived kind. The prefixes of [`Kind::Crafters`] overlap those of the entity
    /// kinds; only the order of [`Kind::ALL`] resolves it. Kinds that share a folder are told
    /// apart by [`Self::by_record`].
    pub(crate) fn by_path(record_path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            kind.rule()
                .is_some_and(|rule| rule.matches_path(record_path))
        })
    }

    /// Returns the kind that selects a Record with `record_path` and the struct `type_name`; the
    /// kinds that share a folder (the V3 and the older loot Records) differ by the struct.
    pub(crate) fn by_record(record_path: &str, type_name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            kind.rule()
                .is_some_and(|rule| rule.selects(record_path, type_name))
        })
    }

    /// Returns whether the kind is derived from the entities of other kinds and selects no Records
    /// of its own.
    #[must_use]
    pub const fn is_derived(self) -> bool {
        matches!(
            self,
            Self::Paints
                | Self::Loadouts
                | Self::HarvestableLocations
                | Self::Shops
                | Self::LootContainers
                | Self::ContractShipSpawns
                | Self::ContractLocationQueries
                | Self::ContractNpcSpawns
                | Self::ContractEntitySpawns
        )
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
