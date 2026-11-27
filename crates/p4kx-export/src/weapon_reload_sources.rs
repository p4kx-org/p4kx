// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::catalog_rows::spool::RowSpoolError;
use crate::catalog_rows::staged::StagedCatalogSequence;
use crate::schema_fields::every_field_required;

use super::actor::PlayerMotion;
use super::assets::{AssetLink, Controller, Definition};
use super::coverage::WeaponAnimationGraphCoverage;
use super::document::Database;
use super::graph::BranchLink;
use super::relation::FragmentMatch;
use super::scope::DatabaseScopes;

/// Physical Player context and Mannequin declarations reached from it.
#[derive(Debug, Default, JsonSchema)]
#[schemars(rename_all = "camelCase", transform = every_field_required)]
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

    fn serialize_graph<S: Serializer>(
        &self,
        fragments: &impl Serialize,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut document = serializer.serialize_struct("WeaponAnimationSources", 8)?;
        document.serialize_field("player", &self.player)?;
        document.serialize_field("actorBindings", &self.actor_bindings)?;
        document.serialize_field("controllers", &self.controllers)?;
        document.serialize_field("databases", &self.databases)?;
        document.serialize_field("tagDefinitions", &self.tag_definitions)?;
        document.serialize_field("scopes", &self.scopes)?;
        document.serialize_field("branches", &self.branches)?;
        document.serialize_field("fragmentSelectors", fragments)?;
        document.end()
    }
}

impl Serialize for WeaponAnimationSources<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.serialize_graph(&self.fragment_selectors, serializer)
    }
}

#[derive(Debug)]
pub(crate) struct StagedWeaponAnimationSources<'source> {
    graph: WeaponAnimationSources<'source>,
    fragment_selectors: StagedCatalogSequence,
}

impl<'source> StagedWeaponAnimationSources<'source> {
    pub(super) const fn new(
        graph: WeaponAnimationSources<'source>,
        fragment_selectors: StagedCatalogSequence,
    ) -> Self {
        Self {
            graph,
            fragment_selectors,
        }
    }

    pub(crate) const fn stored_bytes(&self) -> u64 {
        self.fragment_selectors.stored_bytes()
    }

    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.fragment_selectors.take_failure()
    }
}

impl Serialize for StagedWeaponAnimationSources<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.fragment_selectors.clear_failure();
        self.graph
            .serialize_graph(&self.fragment_selectors, serializer)
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
