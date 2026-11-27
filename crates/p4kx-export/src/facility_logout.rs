// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_point;
use super::super::interaction_source::{InteractionList, WeakLink};
use super::super::shared_interaction;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const COMPONENT: &str = "LogoutComponentParams";
const INTERACTION_SLOT: &str = "LogoutInteractionParams";

/// The authored interactions that support logging out and respawning at a facility.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Logout<'source> {
    logout_interactions: InteractionList<LogoutSlot<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> Logout<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let logout_interactions = source.strong_pointer_family_array_to(
            "logoutInteractions",
            INTERACTION_SLOT,
            notes,
            LogoutSlot::read,
        )?;
        let additional_fields = source
            .authored_members_except([("logoutInteractions", logout_interactions.state())])?;
        Ok(Self {
            logout_interactions,
            additional_fields,
        })
    }
}

/// The original targets of one stored logout or respawn interaction slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LogoutSlot<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    logout_interaction: WeakLink<'source>,
    logout_interaction_point: WeakLink<'source>,
    respawn_interaction: WeakLink<'source>,
    respawn_interaction_point: WeakLink<'source>,
}

impl<'source> LogoutSlot<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let logout_interaction =
            source.weak_link_to("logoutInteraction", shared_interaction::SOURCE_TYPE, notes)?;
        let logout_interaction_point = source.weak_link_to(
            "logoutInteractionPoint",
            interaction_point::SOURCE_TYPE,
            notes,
        )?;
        let respawn_interaction =
            source.weak_link_to("respawnInteraction", shared_interaction::SOURCE_TYPE, notes)?;
        let respawn_interaction_point = source.weak_link_to(
            "respawnInteractionPoint",
            interaction_point::SOURCE_TYPE,
            notes,
        )?;
        let source = UnitSource::read(
            source,
            [
                ("logoutInteraction", logout_interaction.state()),
                ("logoutInteractionPoint", logout_interaction_point.state()),
                ("respawnInteraction", respawn_interaction.state()),
                ("respawnInteractionPoint", respawn_interaction_point.state()),
            ],
        )?;
        Ok(Self {
            source,
            logout_interaction,
            logout_interaction_point,
            respawn_interaction,
            respawn_interaction_point,
        })
    }
}
