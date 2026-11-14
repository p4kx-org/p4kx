// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const COMPONENT: &str = "EntityComponentMissionBoardUIProvider";
const BOARD_BASE: &str = "MissionBoardType_Base";
const BOARD_CAREER: &str = "MissionBoardType_Career";
const BOARD_NOTICE: &str = "MissionBoardType_Notice";
const FACTION_REPUTATION: &str = "FactionReputation";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// An authored mission board on a physical Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MissionBoard<'source> {
    pub(super) owner: ComponentOwner,
    board_type: SourcedValue<BoardType<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionBoard<'source> {
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct BoardType<'source> {
    source_type: SourcedValue<&'source str>,
    details: BoardDetails<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> BoardType<'source> {
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum BoardDetails<'source> {
    Base,
    Career {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        guild_faction_reputation: RecordLink<'source>,
        #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
        factions: SourcedValue<Vec<RecordLink<'source>>>,
    },
    Notice,
    Authored,
}
