// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::Identity;
use crate::audio_control_payload::AtlPosition;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SubDatabaseSelectorKind")]
pub(super) enum SelectorKind {
    Tags,
    FragmentName,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SubDatabaseScopeState")]
pub(super) enum ScopeState {
    Complete,
    MissingDefinitionSelector,
    DuplicateDefinitionSelector,
    InvalidDefinitionPath,
    MissingDefinition,
    AmbiguousDefinition,
    NotSelected,
    Unavailable,
    Unreadable,
    InvalidDefinition,
    IncompleteImports,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SubDatabaseSelectorState")]
pub(super) enum SelectorState {
    Missing,
    Empty,
    Repeated,
    Present,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SubDatabaseResolution")]
pub(super) enum Resolution {
    InvalidToken,
    UnsupportedSpelling,
    Incomplete,
    Missing,
    Resolved,
    Ambiguous,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SubDatabaseSelector")]
pub(super) struct Selector {
    pub(super) branch: DecimalU64,
    pub(super) kind: SelectorKind,
    pub(super) position: AtlPosition,
    pub(super) state: SelectorState,
    pub(super) scope: Option<ScopeState>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SubDatabaseToken")]
pub(super) struct Token<'source> {
    pub(super) spelling: &'source str,
    pub(super) position: AtlPosition,
    pub(super) resolution: Resolution,
    pub(super) candidate_count: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SubDatabaseCandidate")]
pub(super) struct Candidate<'source> {
    #[schemars(schema_with = "crate::schema::identity_definition")]
    pub(super) target: &'source Identity,
    pub(super) name: &'source str,
    pub(super) position: AtlPosition,
    pub(super) name_position: AtlPosition,
}
