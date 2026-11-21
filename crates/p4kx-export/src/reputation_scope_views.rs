// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{ScopeDefinition, ScopeStandingMap, StandingLink};
use crate::catalog_text_contract::GameTextRef;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::SourceArenaOutput;
use p4kx_extraction::SourcedValue;

impl<'source> ScopeDefinition<'source> {
    /// The original technical scope name.
    #[must_use]
    pub const fn scope_name(&self) -> &SourcedValue<&'source str> {
        &self.scope_name
    }

    /// The original protected description reference.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// The original icon asset path.
    #[must_use]
    pub const fn icon(&self) -> &SourcedValue<&'source str> {
        &self.icon
    }

    /// The complete owned standing map with its original source state.
    #[must_use]
    pub const fn standing_map(&self) -> &SourcedValue<ScopeStandingMap<'source>> {
        &self.standing_map
    }

    /// The remaining original scope field occurrences.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> ScopeStandingMap<'source> {
    /// The concrete original owned structure type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The original reputation ceiling with exact numeric storage.
    #[must_use]
    pub const fn reputation_ceiling(&self) -> &SourcedValue<ResourceNumber> {
        &self.reputation_ceiling
    }

    /// The original initial reputation with exact numeric storage.
    #[must_use]
    pub const fn initial_reputation(&self) -> &SourcedValue<ResourceNumber> {
        &self.initial_reputation
    }

    /// The authored standing slots without discarding duplicates or nulls.
    #[must_use]
    pub const fn standings(&self) -> &SourcedValue<Vec<StandingLink<'source>>> {
        &self.standings
    }

    /// The remaining original standing-map field occurrences.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}
