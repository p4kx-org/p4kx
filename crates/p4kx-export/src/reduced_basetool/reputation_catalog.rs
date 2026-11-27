// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::faction_reputations::{self, Dependencies};
use super::mission_boards;
use super::record_targets::RecordTargets;
use super::reputation_contexts;
use super::reputation_perks::{self, RankTargets};
use super::reputation_ranks::{self, Ranks};
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_queries::{ReputationSelection, ReputationSnapshot};
use crate::catalog_rows::staged::StagedCatalogTable;

pub(super) struct Reputation {
    pub(super) factions: StagedCatalogTable,
    pub(super) faction_targets: RecordTargets,
    pub(super) scopes: StagedCatalogTable,
    pub(super) standings: StagedCatalogTable,
    pub(super) contexts: StagedCatalogTable,
    pub(super) perks: StagedCatalogTable,
    pub(super) mission_boards: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Reputation, ReducedError> {
    let selection = ReputationSelection {
        faction_reputations: Some(CatalogRecordSelection::All),
        contexts: Some(CatalogRecordSelection::All),
        perks: Some(CatalogRecordSelection::All),
        ..ReputationSelection::ranks()
    };
    let staged = parsed.with_reputation(selection, cancel, |snapshot| {
        stage(&snapshot, scratch, cancel, texts)
    })??;
    let mission_boards = mission_boards::collect(parsed, scratch, cancel, &staged.faction_targets)?;
    Ok(Reputation {
        factions: staged.factions,
        faction_targets: staged.faction_targets,
        scopes: staged.scopes,
        standings: staged.standings,
        contexts: staged.contexts,
        perks: staged.perks,
        mission_boards,
    })
}

struct StagedReputation {
    factions: StagedCatalogTable,
    faction_targets: RecordTargets,
    scopes: StagedCatalogTable,
    standings: StagedCatalogTable,
    contexts: StagedCatalogTable,
    perks: StagedCatalogTable,
}

fn stage(
    source: &ReputationSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedReputation, ReducedError> {
    let mut references = TextRefs::default();
    let Ranks {
        scopes,
        scope_targets,
        standings,
        standing_targets,
    } = reputation_ranks::stage(source, scratch, cancel, &mut references)?;
    let context_rows = source
        .contexts
        .as_ref()
        .ok_or(ReducedError::IncompleteScope(reputation_contexts::PATH))?;
    let (contexts, context_targets) =
        reputation_contexts::stage(context_rows, &scope_targets, scratch, cancel)?;
    let perk_rows = source
        .perks
        .as_ref()
        .ok_or(ReducedError::IncompleteScope(reputation_perks::PATH))?;
    let perks = reputation_perks::stage(
        perk_rows,
        &RankTargets {
            scopes: &scope_targets,
            standings: &standing_targets,
        },
        scratch,
        cancel,
        &mut references,
    )?;
    let faction_rows = source
        .faction_reputations
        .as_ref()
        .ok_or(ReducedError::IncompleteScope(faction_reputations::PATH))?;
    let factions = faction_reputations::stage(
        faction_rows,
        &Dependencies {
            scopes: &scope_targets,
            standings: &standing_targets,
            contexts: &context_targets,
            perks: &perks.targets,
        },
        scratch,
        cancel,
        &mut references,
    )?;
    texts.add(references, &source.public_names, &source.game_texts, cancel)?;
    Ok(StagedReputation {
        factions: factions.table,
        faction_targets: factions.targets,
        scopes,
        standings,
        contexts,
        perks: perks.table,
    })
}
