// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::definition_reference::DefinitionReference;
use crate::mission_property_clue::InvestigationClueItem;
use crate::mission_property_name::InvestigationName;

/// Authored investigation configuration without computing runtime suspects or locations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct Investigation<'f> {
    /// The initial journal-entry References in source order.
    pub(crate) starting_journal_entries: Option<Vec<Option<DefinitionReference>>>,
    /// Suspects in source order, retaining null declarations.
    pub(crate) suspects: Option<Vec<Option<InvestigationSuspect<'f>>>>,
    /// Location configurations in source order, retaining null declarations.
    pub(crate) locations: Option<Vec<Option<InvestigationLocation<'f>>>>,
}

/// One authored investigation suspect.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct InvestigationSuspect<'f> {
    /// The authored name configuration.
    pub(crate) name: Option<InvestigationName<'f>>,
    /// Whether this is an initial suspect.
    pub(crate) initial_suspect: Option<bool>,
    /// The authored guilt flag, without deriving a result.
    pub(crate) is_guilty: Option<bool>,
}

/// The clue-item configuration of an investigation location.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct InvestigationLocation<'f> {
    /// Clue items in source order, retaining null pointers.
    pub(crate) clue_items: Option<Vec<Option<InvestigationClueItem<'f>>>>,
}
