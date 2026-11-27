// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_text_contract::GameTextRef;
use crate::definition_reference::DefinitionReference;
use crate::mission_property_name::InvestigationName;
use crate::mission_property_set::MissionPropertyText;
use crate::mission_property_source::SourceTagList;
use crate::owned_reference::OwnedReference;

#[path = "mission_property_clue_text.rs"]
mod text;

/// An investigation clue item and its complete concrete source payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct InvestigationClueItem<'f> {
    /// The declared item Entity without following its Record.
    pub entity_class: Option<DefinitionReference>,
    /// The authored spawn tags.
    pub spawn_tags: Option<SourceTagList>,
    /// Ordered clue declarations, retaining null pointers.
    pub clues: Option<Vec<Option<InvestigationClue<'f>>>>,
    /// The concrete subtype-specific fields.
    pub details: ClueItemDetails,
}

/// The subtype-specific source fields of an investigation clue item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ClueItemDetails {
    /// `Investigation_ClueItem` has no subtype fields.
    Base,
    /// `Investigation_ClueItem_Corpse`.
    #[serde(rename_all = "camelCase")]
    Corpse {
        /// The owner-local suspect pointer with its raw target identity.
        suspect_represented: Option<OwnedReference>,
    },
    /// `Investigation_ClueItem_Datapad`.
    Datapad {
        /// The authored Building Blocks canvas Reference.
        canvas: Option<DefinitionReference>,
    },
}

/// An investigation clue and its complete concrete source payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct InvestigationClue<'f> {
    /// The authored long objective text.
    #[schemars(with = "SourcedValue<GameTextRef>", extend("x-gameText" = true))]
    pub objective_long_description: MissionPropertyText<'f>,
    /// The authored short objective text.
    #[schemars(with = "SourcedValue<GameTextRef>", extend("x-gameText" = true))]
    pub objective_short_description: MissionPropertyText<'f>,
    /// The concrete subtype-specific fields.
    pub details: ClueDetails<'f>,
}

/// The subtype-specific source fields of an investigation clue.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ClueDetails<'f> {
    /// `Investigation_Clue` has no subtype fields.
    Base,
    /// `Investigation_Clue_CorpseScanInfo`.
    #[serde(rename_all = "camelCase")]
    CorpseScanInfo {
        /// The authored name configuration.
        name: Option<Box<InvestigationName<'f>>>,
        /// The authored cause-of-death text.
        #[schemars(with = "SourcedValue<GameTextRef>", extend("x-gameText" = true))]
        cause_of_death: MissionPropertyText<'f>,
    },
    /// `Investigation_Clue_JournalEntry`.
    #[serde(rename_all = "camelCase")]
    JournalEntry {
        /// The authored journal-entry Reference.
        journal_entry: Option<DefinitionReference>,
    },
}
