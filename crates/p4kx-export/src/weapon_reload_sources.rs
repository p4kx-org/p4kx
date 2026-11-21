// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

use super::actor::PlayerMotion;
use super::assets::{AssetLink, Controller, Definition};
use super::coverage::WeaponAnimationGraphCoverage;
use super::document::Database;
use super::graph::BranchLink;
use super::relation::FragmentMatch;
use super::scope::DatabaseScopes;

/// Physical Player context and Mannequin declarations reached from it.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct WeaponAnimationSources<'source> {
    /// Player Record candidates and original component slots.
    pub player: Option<SourcedValue<Vec<PlayerMotion<'source>>>>,
    /// Physical Player component bindings.
    pub actor_bindings: Vec<ActorBinding>,
    /// Reached original controller documents.
    pub controllers: Vec<Controller>,
    /// Reached original `AnimDB` documents.
    pub databases: Vec<Database>,
    /// Reached original `TagDef` and `FragDef` documents.
    pub tag_definitions: Vec<Definition>,
    /// Tag and fragment scopes for each reached database.
    pub scopes: Vec<DatabaseScopes>,
    /// Original `SubADB` selectors and traversal outcomes.
    pub branches: Vec<BranchLink>,
    /// Authored fragment selectors and matches within the reached scopes.
    pub fragment_selectors: Vec<FragmentMatch>,
}

impl WeaponAnimationSources<'_> {
    pub(crate) fn graph_coverage(&self, player_candidates: usize) -> WeaponAnimationGraphCoverage {
        WeaponAnimationGraphCoverage::read(self, player_candidates)
    }
}

/// One Player animation component's controller and database locators.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ActorBinding {
    /// Physical Player Record index.
    pub record: SourcedValue<u32>,
    /// Original component ordinal within the Player Record.
    pub component: SourcedValue<u32>,
    /// Original controller selector and its Entry read status.
    pub controller: AssetLink,
    /// Original third-person database selector and its Entry read status.
    pub database: AssetLink,
}
