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
#[non_exhaustive]
pub struct Investigation<'f> {
    /// The initial journal-entry References in source order.
    pub starting_journal_entries: Option<Vec<Option<DefinitionReference>>>,
    /// Suspects in source order, retaining null declarations.
    pub suspects: Option<Vec<Option<InvestigationSuspect<'f>>>>,
    /// Location configurations in source order, retaining null declarations.
    pub locations: Option<Vec<Option<InvestigationLocation<'f>>>>,
}

/// One authored investigation suspect.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct InvestigationSuspect<'f> {
    /// The authored name configuration.
    pub name: Option<Box<InvestigationName<'f>>>,
    /// Whether this is an initial suspect.
    pub initial_suspect: Option<bool>,
    /// The authored guilt flag, without deriving a result.
    pub is_guilty: Option<bool>,
}

/// The clue-item configuration of an investigation location.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct InvestigationLocation<'f> {
    /// Clue items in source order, retaining null pointers.
    pub clue_items: Option<Vec<Option<InvestigationClueItem<'f>>>>,
}
