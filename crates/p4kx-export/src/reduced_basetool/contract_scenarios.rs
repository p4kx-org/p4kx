// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::Kind;
use crate::catalog_queries::contract_scenarios::{
    ContractScenario, ContractScenarioSnapshot, ScenarioFactionTiers, ScenarioProgression,
    ScenarioTier,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogSequence};
use crate::source_value_catalog::SourceRecordReference;

pub(super) const PATH: &str = "contractScenarios";

pub(super) struct Dependencies<'staged> {
    pub(super) factions: &'staged RecordTargets,
    pub(super) reputations: &'staged RecordTargets,
    pub(super) pools: &'staged RecordTargets,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    dependencies: &Dependencies<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogSequence, ReducedError> {
    parsed.with_contract_scenarios(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(&snapshot, dependencies, scratch, cancel)
    })?
}

fn stage(
    snapshot: &ContractScenarioSnapshot<'_>,
    dependencies: &Dependencies<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogSequence, ReducedError> {
    completeness::inventory(PATH, snapshot.availability, &snapshot.coverage)?;
    let mut targets = RecordTargets::default();
    for row in &snapshot.rows {
        checkpoint(cancel)?;
        targets.insert(PATH, row.source())?;
    }
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for row in &snapshot.rows {
        checkpoint(cancel)?;
        output.append(&Scenario::read(row, dependencies, &targets)?)?;
    }
    let sequence = output.finish_sequence()?;
    completeness::rows(PATH, snapshot.rows.len(), sequence.len())?;
    Ok(sequence)
}

type Ordered<T> = Cell<Vec<Cell<T>>>;
type Link<'source> = Cell<RecordLink<'source>>;
type LinkedValue<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scenario<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    faction_reward_tiers: Ordered<FactionTiers<'read>>,
}

impl<'read> Scenario<'read> {
    fn read(
        row: &'read ContractScenario<'_>,
        dependencies: &Dependencies<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            identity: row.source().into(),
            faction_reward_tiers: completeness::list(PATH, row.faction_reward_tiers(), |slot| {
                FactionTiers::read(slot, dependencies, targets)
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FactionTiers<'read> {
    faction: Link<'read>,
    faction_name: Cell<&'read str>,
    faction_reputation: Link<'read>,
    points_as_percent: Cell<&'read bool>,
    progressions: Ordered<Progression<'read>>,
}

impl<'read> FactionTiers<'read> {
    fn read(
        row: &'read ScenarioFactionTiers<'_>,
        dependencies: &Dependencies<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            faction: reference(row.faction(), Kind::Factions, dependencies.factions)?,
            faction_name: completeness::cell(PATH, row.faction_name())?.map(|name| *name),
            faction_reputation: completeness::cell(PATH, row.faction_reputation())?.try_map(
                |link| {
                    RecordLink::read(
                        PATH,
                        link,
                        Kind::FactionReputations,
                        dependencies.reputations,
                    )
                },
            )?,
            points_as_percent: completeness::cell(PATH, row.points_as_percent())?,
            progressions: completeness::list(PATH, row.progressions(), |progression| {
                Progression::read(progression, dependencies.pools, targets)
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Progression<'read> {
    min_multiplier_threshold: Cell<Number<'read>>,
    completion_type: Cell<&'read str>,
    period_mission_scenario: Link<'read>,
    tiers: Ordered<Tier<'read>>,
}

impl<'read> Progression<'read> {
    fn read(
        row: &'read ScenarioProgression<'_>,
        pools: &RecordTargets,
        scenarios: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            min_multiplier_threshold: completeness::cell(PATH, row.min_multiplier_threshold())?
                .map(Number::from),
            completion_type: completeness::cell(PATH, row.completion_type())?.map(|label| *label),
            period_mission_scenario: reference(
                row.period_mission_scenario(),
                Kind::ContractScenarios,
                scenarios,
            )?,
            tiers: completeness::list(PATH, row.tiers(), |tier| Tier::read(tier, pools))?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tier<'read> {
    min_points: Cell<Number<'read>>,
    badge_to_award: Cell<&'read str>,
    global_points_multiplier: Cell<Number<'read>>,
    blueprint_pools: Ordered<RecordLink<'read>>,
}

impl<'read> Tier<'read> {
    fn read(row: &'read ScenarioTier<'_>, pools: &RecordTargets) -> Result<Self, ReducedError> {
        Ok(Self {
            min_points: completeness::cell(PATH, row.min_points())?.map(Number::from),
            badge_to_award: completeness::cell(PATH, row.badge_to_award())?.map(|name| *name),
            global_points_multiplier: completeness::cell(PATH, row.global_points_multiplier())?
                .map(Number::from),
            blueprint_pools: completeness::list(PATH, row.blueprint_pools(), |link| {
                RecordLink::read_source(PATH, link, pools, |name| {
                    name == Kind::BlueprintPools.records().type_name
                })
            })?,
        })
    }
}

fn reference<'read>(
    value: &'read LinkedValue<'_>,
    kind: Kind,
    targets: &RecordTargets,
) -> Result<Link<'read>, ReducedError> {
    completeness::cell(PATH, value)?.try_map(|link| {
        RecordLink::read_source(PATH, link, targets, |name| name == kind.records().type_name)
    })
}
