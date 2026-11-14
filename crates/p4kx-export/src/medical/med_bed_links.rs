// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValueState;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::interaction_source::WeakLink;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const INTERACTION: &str = "SSharedInteractionParams";
const POINT: &str = "SInteractionPointParams";
const CHANNEL: &str = "UsableUseChannelInstance";
const STATE: &str = "SInteractionState";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MedBedLinks<'source> {
    heal_interaction: WeakLink<'source>,
    set_respawn_interaction: WeakLink<'source>,
    cancel_respawn_interaction: WeakLink<'source>,
    cancel_all_respawns_interaction: WeakLink<'source>,
    respawn_interaction: WeakLink<'source>,
    respawn_interaction_point: WeakLink<'source>,
    use_channel_to_heal_actor: WeakLink<'source>,
    surgery_sequence_state: WeakLink<'source>,
}

impl<'source> MedBedLinks<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            heal_interaction: source.weak_link_to("healInteraction", INTERACTION, notes)?,
            set_respawn_interaction: source.weak_link_to(
                "setRespawnInteraction",
                INTERACTION,
                notes,
            )?,
            cancel_respawn_interaction: source.weak_link_to(
                "cancelRespawnInteraction",
                INTERACTION,
                notes,
            )?,
            cancel_all_respawns_interaction: source.weak_link_to(
                "cancelAllRespawnsInteraction",
                INTERACTION,
                notes,
            )?,
            respawn_interaction: source.weak_link_to("respawnInteraction", INTERACTION, notes)?,
            respawn_interaction_point: source.weak_link_to(
                "respawnInteractionPoint",
                POINT,
                notes,
            )?,
            use_channel_to_heal_actor: source.weak_link_to(
                "useChannelToHealActor",
                CHANNEL,
                notes,
            )?,
            surgery_sequence_state: source.weak_link_to("surgerySequenceState", STATE, notes)?,
        })
    }

    pub(super) const fn members(&self) -> [(&'static str, SourceValueState); 8] {
        [
            ("healInteraction", self.heal_interaction.state()),
            (
                "setRespawnInteraction",
                self.set_respawn_interaction.state(),
            ),
            (
                "cancelRespawnInteraction",
                self.cancel_respawn_interaction.state(),
            ),
            (
                "cancelAllRespawnsInteraction",
                self.cancel_all_respawns_interaction.state(),
            ),
            ("respawnInteraction", self.respawn_interaction.state()),
            (
                "respawnInteractionPoint",
                self.respawn_interaction_point.state(),
            ),
            (
                "useChannelToHealActor",
                self.use_channel_to_heal_actor.state(),
            ),
            ("surgerySequenceState", self.surgery_sequence_state.state()),
        ]
    }
}
