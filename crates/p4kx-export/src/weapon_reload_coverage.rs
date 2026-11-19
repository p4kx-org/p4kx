// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use crate::catalog_read::original::PhysicalEntryIdentity;

use super::WeaponAnimationSources;
use super::assets::AssetLink;
use super::graph::BranchVisit;

/// Observations about the Player context and the reached animation graph.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct WeaponAnimationGraphCoverage {
    /// The observed Player selector state, or no value when the Item needed no graph.
    pub player_state: Option<SourceValueState>,
    /// Physical Player Records reached by the authored class lookup.
    pub player_candidates: usize,
    /// Player component arrays or occurrences with incomplete source evidence.
    pub incomplete_player_components: usize,
    /// Player action-controller selectors without a proved readable target.
    pub unresolved_controllers: usize,
    /// Player third-person database selectors without a proved readable target.
    pub unresolved_databases: usize,
    /// Reached branch selectors without a proved readable target database.
    pub unresolved_branches: usize,
    /// Reached branches whose target was already active in the graph.
    pub cyclic_branches: usize,
    /// Reached databases without both complete original tag scopes.
    pub incomplete_scopes: usize,
}

impl WeaponAnimationGraphCoverage {
    pub(super) fn read(sources: &WeaponAnimationSources<'_>, player_candidates: usize) -> Self {
        let mut result = Self {
            player_state: sources.player.as_ref().map(SourcedValue::state),
            player_candidates,
            ..Self::default()
        };
        if let Some(players) = sources.player.as_ref().and_then(|player| player.value()) {
            for player in players {
                result.incomplete_player_components += usize::from(
                    !player.components.origin().source_set_complete()
                        || !matches!(
                            player.components.state(),
                            SourceValueState::Present
                                | SourceValueState::Absent
                                | SourceValueState::Null
                        ),
                );
                for slot in player.components.value().into_iter().flatten() {
                    result.incomplete_player_components += usize::from(
                        !slot.origin().source_set_complete()
                            || !matches!(
                                slot.state(),
                                SourceValueState::Present | SourceValueState::Null
                            ),
                    );
                }
            }
        }
        for actor in &sources.actor_bindings {
            let controllers = sources.controllers.iter().map(|asset| &asset.source);
            let databases = sources.databases.iter().map(|asset| &asset.source);
            result.unresolved_controllers +=
                usize::from(!has_target(&actor.controller, controllers));
            result.unresolved_databases += usize::from(!has_target(&actor.database, databases));
        }
        for branch in &sources.branches {
            match branch.visit() {
                Some(BranchVisit::Resolved) => {}
                Some(BranchVisit::Cycle) => result.cyclic_branches += 1,
                None | Some(BranchVisit::Unresolved) => result.unresolved_branches += 1,
            }
        }
        for scope in &sources.scopes {
            result.incomplete_scopes += usize::from(
                [scope.tags.coverage(), scope.fragments.coverage()]
                    .into_iter()
                    .any(|scope| {
                        scope.value() != Some(&true) || !scope.origin().source_set_complete()
                    }),
            );
        }
        result
    }
}

fn has_target<'source>(
    link: &AssetLink,
    mut targets: impl Iterator<Item = &'source PhysicalEntryIdentity>,
) -> bool {
    link.is_read()
        && link.source.as_ref().is_some_and(|source| {
            targets.any(|target| {
                target.entry_index() == source.entry_index()
                    && target.input_ref() == source.input_ref()
            })
        })
}
