// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    BlackboardProperty, FactionReputationDefinition, FactionReputationPresentation,
    FactionReputationRelations, FactionText, ReputationRelation,
};
use crate::catalog_text_contract::GameTextRef;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use p4kx_extraction::{DecimalU64, SourcedValue};

impl<'source> FactionReputationDefinition<'source> {
    /// Borrows the original presentation without flattening its source states.
    #[must_use]
    pub const fn presentation(&self) -> &FactionReputationPresentation<'source> {
        &self.presentation
    }

    /// Borrows the original links and allied or hostile rules.
    #[must_use]
    pub const fn relations(&self) -> &FactionReputationRelations<'source> {
        &self.relations
    }

    /// Borrows the ordered original blackboard property slots.
    #[must_use]
    pub const fn properties_bb(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<BlackboardProperty<'source>>>> {
        &self.properties_bb
    }

    /// Borrows remaining original field occurrences and their typed values.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

impl<'source> FactionReputationPresentation<'source> {
    /// The authored technical reputation name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// The protected display-name reference.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<GameTextRef> {
        &self.display_name
    }

    /// The protected description as an original locale reference or literal.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<FactionText<'source, GameTextRef>> {
        &self.description
    }

    /// The original protected background string.
    #[must_use]
    pub const fn background(&self) -> &SourcedValue<&'source str> {
        &self.background
    }

    /// The complete authored unsigned identifier.
    #[must_use]
    pub const fn geid(&self) -> &SourcedValue<DecimalU64> {
        &self.geid
    }

    /// The original logo asset path.
    #[must_use]
    pub const fn logo(&self) -> &SourcedValue<&'source str> {
        &self.logo
    }

    /// The original mission-board logo asset path.
    #[must_use]
    pub const fn mission_board_logo(&self) -> &SourcedValue<&'source str> {
        &self.mission_board_logo
    }

    /// The original guild-banner asset path.
    #[must_use]
    pub const fn guild_banner(&self) -> &SourcedValue<&'source str> {
        &self.guild_banner
    }

    /// The authored application-visibility flag and its source state.
    #[must_use]
    pub const fn hide_in_delpih_app(&self) -> &SourcedValue<bool> {
        &self.hide_in_delpih_app
    }

    /// The authored NPC flag without replacing absent with false.
    #[must_use]
    pub const fn is_npc(&self) -> &SourcedValue<bool> {
        &self.is_npc
    }
}

impl<'source> FactionReputationRelations<'source> {
    /// The original UI-context reference and its physical target.
    #[must_use]
    pub const fn reputation_context_properties_ui(
        &self,
    ) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.reputation_context_properties_ui
    }

    /// The original ally references in authored order.
    #[must_use]
    pub const fn allies(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.allies
    }

    /// The original enemy references, including dangling targets.
    #[must_use]
    pub const fn enemies(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.enemies
    }

    /// The original hostile-standing rule.
    #[must_use]
    pub const fn hostility_params(&self) -> &SourcedValue<ReputationRelation<'source>> {
        &self.hostility_params
    }

    /// The original allied-standing rule.
    #[must_use]
    pub const fn allied_params(&self) -> &SourcedValue<ReputationRelation<'source>> {
        &self.allied_params
    }

    /// The original ordered perk-list references.
    #[must_use]
    pub const fn perk_reputation_reward_list(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>> {
        &self.perk_reputation_reward_list
    }
}
