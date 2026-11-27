// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::blueprint_catalog::{self, BlueprintCatalog};
use super::blueprint_reward_sources;
use super::contract_scenarios::{self, Dependencies};
use super::contract_scope;
use super::contracts;
use super::factions;
use super::gameplay_properties;
use super::record_targets::RecordTargets;
use super::reputation_catalog;
use super::selected_tables::SelectedTables;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};

pub(super) fn stage(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
) -> Result<(), ReducedError> {
    let pools = stage_blueprints(parsed, scratch, cancel, tables, texts)?;
    stage_reputation(parsed, scratch, cancel, tables, texts, &pools)?;
    stage_contracts(parsed, scratch, cancel, tables, texts, &pools)?;
    tables.table(
        "definitions.contractScopes",
        contract_scope::collect(parsed, scratch, cancel, texts)?,
    )?;
    tables.table(
        gameplay_properties::PATH,
        gameplay_properties::collect(parsed, scratch, cancel, texts)?,
    )
}

fn stage_blueprints(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
) -> Result<RecordTargets, ReducedError> {
    let BlueprintCatalog {
        definitions,
        presentations,
        tiers,
        categories,
        pools,
        pool_weights,
        pool_targets,
        dismantle_processes,
    } = blueprint_catalog::collect(parsed, scratch, cancel, texts)?;
    tables.table("definitions.blueprints", definitions)?;
    tables.table("projections.blueprintPresentation", presentations)?;
    tables.table("configurations.blueprintTiers", tiers)?;
    tables.table("definitions.blueprintCategories", categories)?;
    tables.table("definitions.blueprintPools", pools)?;
    tables.table("projections.blueprintPoolWeights", pool_weights)?;
    tables.table("definitions.dismantleProcesses", dismantle_processes)?;
    Ok(pool_targets)
}

fn stage_reputation(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
    pools: &RecordTargets,
) -> Result<(), ReducedError> {
    let ranks = reputation_catalog::collect(parsed, scratch, cancel, texts)?;
    let factions = factions::collect(parsed, &ranks.faction_targets, scratch, cancel, texts)?;
    let scenarios = contract_scenarios::collect(
        parsed,
        &Dependencies {
            factions: &factions.targets,
            reputations: &ranks.faction_targets,
            pools,
        },
        scratch,
        cancel,
    )?;
    tables.table("definitions.factionReputations", ranks.factions)?;
    tables.table("definitions.reputationScopes", ranks.scopes)?;
    tables.table("definitions.reputationStandings", ranks.standings)?;
    tables.table("definitions.reputationContexts", ranks.contexts)?;
    tables.table("definitions.reputationPerks", ranks.perks)?;
    tables.table("configurations.missionBoards", ranks.mission_boards)?;
    tables.table(factions::PATH, factions.table)?;
    tables.sequence(contract_scenarios::PATH, scenarios)
}

fn stage_contracts(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
    pools: &RecordTargets,
) -> Result<(), ReducedError> {
    parsed.with_contracts(CatalogRecordSelection::All, cancel, |snapshot| {
        let selected = contracts::stage(&snapshot, scratch, cancel, texts)?;
        let rewards =
            blueprint_reward_sources::collect(parsed, &snapshot, pools, scratch, cancel, texts)?;
        tables.table(contracts::PATH, selected.rows)?;
        tables.table("projections.missionTitles", selected.titles)?;
        tables.table("declarations.blueprintRewardSources", rewards)
    })?
}
