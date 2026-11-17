// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{StandingDefinition, StandingPresentation, StandingProgression};
use crate::catalog_text_contract::GameTextRef;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::SourceArenaOutput;
use p4kx_extraction::SourcedValue;

impl<'source> StandingDefinition<'source> {
    /// The complete original presentation fields.
    #[must_use]
    pub const fn presentation(&self) -> &StandingPresentation<'source> {
        &self.presentation
    }

    /// The original progression values without unit conversion.
    #[must_use]
    pub const fn progression(&self) -> &StandingProgression {
        &self.progression
    }

    /// The remaining original standing field occurrences.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl<'source> StandingPresentation<'source> {
    /// The authored technical standing name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// The original protected literal description.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<&'source str> {
        &self.description
    }

    /// The original protected display-name reference.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<GameTextRef> {
        &self.display_name
    }

    /// The original protected perk-description reference.
    #[must_use]
    pub const fn perk_description(&self) -> &SourcedValue<GameTextRef> {
        &self.perk_description
    }

    /// The original standing icon asset path.
    #[must_use]
    pub const fn icon(&self) -> &SourcedValue<&'source str> {
        &self.icon
    }

    /// The original mission-board icon asset path.
    #[must_use]
    pub const fn mission_board_icon(&self) -> &SourcedValue<&'source str> {
        &self.mission_board_icon
    }
}

impl StandingProgression {
    /// The original minimum reputation with its exact numeric storage.
    #[must_use]
    pub const fn min_reputation(&self) -> &SourcedValue<ResourceNumber> {
        &self.min_reputation
    }

    /// The original drift amount with its exact numeric storage.
    #[must_use]
    pub const fn drift_reputation(&self) -> &SourcedValue<ResourceNumber> {
        &self.drift_reputation
    }

    /// The original number of hours between reputation drifts.
    #[must_use]
    pub const fn drift_time_hours(&self) -> &SourcedValue<ResourceFloat32> {
        &self.drift_time_hours
    }

    /// The original progression gate flag and its state.
    #[must_use]
    pub const fn gated(&self) -> &SourcedValue<bool> {
        &self.gated
    }
}
