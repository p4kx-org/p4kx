// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::LocalisedText;
use crate::definition_reference::DefinitionReference;
use crate::mission_property_name::InvestigationName;
use crate::mission_property_source::SourceTagList;
use crate::owned_reference::OwnedReference;

/// An investigation clue item and its complete concrete source payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct InvestigationClueItem<'f> {
    /// The declared item Entity without following its Record.
    pub(crate) entity_class: Option<DefinitionReference>,
    /// The authored spawn tags.
    pub(crate) spawn_tags: Option<SourceTagList>,
    /// Ordered clue declarations, retaining null pointers.
    pub(crate) clues: Option<Vec<Option<InvestigationClue<'f>>>>,
    /// The concrete subtype-specific fields.
    pub(crate) details: ClueItemDetails,
}

/// The subtype-specific source fields of an investigation clue item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ClueItemDetails {
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
pub(crate) struct InvestigationClue<'f> {
    /// The authored long objective text.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) objective_long_description: LocalisedText<'f>,
    /// The authored short objective text.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) objective_short_description: LocalisedText<'f>,
    /// The concrete subtype-specific fields.
    pub(crate) details: ClueDetails<'f>,
}

/// The subtype-specific source fields of an investigation clue.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ClueDetails<'f> {
    /// `Investigation_Clue` has no subtype fields.
    Base,
    /// `Investigation_Clue_CorpseScanInfo`.
    #[serde(rename_all = "camelCase")]
    CorpseScanInfo {
        /// The authored name configuration.
        name: Option<Box<InvestigationName<'f>>>,
        /// The authored cause-of-death text.
        #[schemars(extend("x-gameText" = true))]
        cause_of_death: LocalisedText<'f>,
    },
    /// `Investigation_Clue_JournalEntry`.
    #[serde(rename_all = "camelCase")]
    JournalEntry {
        /// The authored journal-entry Reference.
        journal_entry: Option<DefinitionReference>,
    },
}
