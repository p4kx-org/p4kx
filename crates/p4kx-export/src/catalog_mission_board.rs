// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_entity_component_read::ComponentKind;
use super::component_sources::physical_component;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const KIND: ComponentKind = ComponentKind {
    source_type: "EntityComponentMissionBoardUIProvider",
    inspection: ConfigurationInspection::MissionBoard,
};
const BOARD_BASE: &str = "MissionBoardType_Base";
const BOARD_CAREER: &str = "MissionBoardType_Career";
const BOARD_NOTICE: &str = "MissionBoardType_Notice";
const FACTION_REPUTATION: &str = "FactionReputation";

physical_component!(MissionBoardComponents, MissionBoard, KIND);

/// An authored mission board on a physical Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionBoard<'source> {
    pub(super) owner: ComponentOwner,
    board_type: SourcedValue<BoardType<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionBoard<'source> {
    /// Borrows the original physical owner and component slot.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the sourced original board-type pointer and its content.
    #[must_use]
    pub const fn board_type(&self) -> &SourcedValue<BoardType<'source>> {
        &self.board_type
    }

    /// Borrows the provider's remaining authored fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let board_type =
            fields.strong_pointer_family_to("boardType", BOARD_BASE, notes, BoardType::read)?;
        let authored_fields =
            fields.authored_members_except([("boardType", board_type.state())])?;
        Ok(Self {
            owner,
            board_type,
            authored_fields,
        })
    }
}

/// One original board type and its variant-specific data.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BoardType<'source> {
    source_type: SourcedValue<&'source str>,
    details: BoardDetails<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> BoardType<'source> {
    /// Borrows the exact original board-type name.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the typed details of the original board variant.
    #[must_use]
    pub const fn details(&self) -> &BoardDetails<'source> {
        &self.details
    }

    /// Borrows original board-type fields outside the typed details.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let details = match fields.source_type()? {
            BOARD_BASE => BoardDetails::Base,
            BOARD_CAREER => BoardDetails::Career {
                guild_faction_reputation: fields.record_link_to(
                    "guildFactionReputation",
                    FACTION_REPUTATION,
                    notes,
                )?,
                factions: fields.record_link_array_to("factions", FACTION_REPUTATION, notes)?,
            },
            BOARD_NOTICE => BoardDetails::Notice,
            _ => {
                fields.note_retained_variant(notes)?;
                BoardDetails::Authored
            }
        };
        let authored_fields = match &details {
            BoardDetails::Career {
                guild_faction_reputation,
                factions,
            } => fields.authored_members_except([
                ("guildFactionReputation", guild_faction_reputation.state()),
                ("factions", factions.state()),
            ])?,
            _ => fields.authored_members()?,
        };
        Ok(Self {
            source_type: fields.source_type_value()?,
            details,
            authored_fields,
        })
    }
}

/// Variants of an authored mission board.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum BoardDetails<'source> {
    /// The original base board type.
    Base,
    /// A career board with its guild and faction references.
    Career {
        /// The original guild reputation reference and its target.
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        guild_faction_reputation: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// The original faction slots in order, including null and unresolved references.
        #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
        factions: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    },
    /// The original notice board type.
    Notice,
    /// Another original type retained in the authored fields.
    Authored,
}
