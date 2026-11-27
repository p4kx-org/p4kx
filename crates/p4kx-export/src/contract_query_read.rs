// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

use super::plan::Plan;
use super::{ContractQueryCoverage, ContractSummary};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::contract_missions::ContractEntry;
use crate::entity_spawn_model::EntitySpawnPool;
use crate::export_ledger::ExportLedger;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_intros::{Grants, PendingContract};
use crate::mission_property_set::{MissionPropertySet, MissionPropertySets};
use crate::mission_titles::MissionTitles;
use crate::missions::record_read::{ContractPiece, GeneratorFacets, Piece};
use crate::npc_spawn_model::PublishedNpcPool;
use crate::spawn_model::{LocationQuery, ShipSpawnPool};
use crate::spawn_pools::SpawnPools;

#[derive(Clone, Copy)]
pub(super) enum Retention {
    Details,
    Summaries,
}

#[derive(Default)]
pub(super) struct Projected<'source> {
    pub(super) summaries: Vec<ContractSummary<'source>>,
    pub(super) rows: Vec<ContractEntry<'source>>,
    pub(super) titles: Vec<MissionTitles<'source>>,
    pub(super) generators: Vec<GeneratorDefinition<'source>>,
    pub(super) dependency_generators: Vec<GeneratorDefinition<'source>>,
    pub(super) property_sets: Vec<MissionPropertySet<'source>>,
    pub(super) ships: Vec<ShipSpawnPool>,
    pub(super) npcs: Vec<PublishedNpcPool>,
    pub(super) entities: Vec<EntitySpawnPool>,
    pub(super) locations: Vec<LocationQuery>,
    pub(super) coverage: ContractQueryCoverage<'source>,
}

pub(super) struct Transaction<'source> {
    pub(super) retention: Retention,
    pub(super) pending: Vec<PendingContract<'source>>,
    pub(super) grants: Grants,
    pub(super) sets: MissionPropertySets<'source>,
    pub(super) pools: SpawnPools<'source>,
    pub(super) ledger: ExportLedger,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    plan: Plan<'source>,
    retention: Retention,
    cancel: Option<&CancelToken>,
) -> Result<Projected<'source>, CatalogError> {
    let Plan {
        roots,
        generators,
        grants,
        mut coverage,
        has_rows,
        target,
        ..
    } = plan;
    let mut output = Projected::default();
    if !has_rows {
        output.coverage = coverage;
        return Ok(output);
    }
    let mut transaction = Transaction {
        retention,
        pending: Vec::new(),
        grants,
        sets: MissionPropertySets::default(),
        pools: SpawnPools::default(),
        ledger: ExportLedger::default(),
    };
    let facets = match retention {
        Retention::Details => GeneratorFacets::Contracts,
        Retention::Summaries => GeneratorFacets::Summaries,
    };
    let mut interpreter = super::interpret::Interpreter::new(forge, projection, cancel, facets)?;
    for (position, tree) in generators {
        interpreter.generator(
            &tree,
            roots.contains(&position),
            &mut transaction,
            &mut output,
            &mut coverage,
        )?;
    }
    let (pools, failures, ledger) = interpreter.finish();
    transaction.merge_pools(pools);
    transaction.ledger.merge(ledger);
    coverage.projection_failures.extend(failures);
    super::finish::finish(
        projection,
        &roots,
        target,
        transaction,
        &mut output,
        &mut coverage,
        cancel,
    )?;
    coverage.dependencies.forward_records.sort_unstable();
    output.coverage = coverage;
    Ok(output)
}

pub(super) fn take_observed<'source>(
    observed: crate::missions::record_read::ObservedPiece<'source>,
    selected: bool,
    transaction: &mut Transaction<'source>,
    output: &mut Projected<'source>,
    coverage: &mut ContractQueryCoverage<'source>,
) -> Result<(), CatalogError> {
    transaction.ledger.merge(observed.ledger);
    if let Some(definition) = observed.definition {
        if selected {
            output.generators.push(definition);
        } else {
            output.dependency_generators.push(definition);
        }
    }
    match observed.piece {
        Piece::Contracts(piece) => transaction.take(*piece)?,
        Piece::Failed(failure) => {
            transaction.grants.incomplete();
            coverage.projection_failures.push(failure);
        }
    }
    Ok(())
}

impl<'source> Transaction<'source> {
    fn take(&mut self, piece: ContractPiece<'source>) -> Result<(), CatalogError> {
        self.sets.merge(piece.property_sets)?;
        self.merge_pools(piece.pools);
        self.pending.extend(piece.contracts);
        self.grants.merge(piece.grants)
    }

    fn merge_pools(&mut self, pools: SpawnPools<'source>) {
        let pools = match self.retention {
            Retention::Details => pools,
            Retention::Summaries => SpawnPools {
                locations: pools.locations,
                ..SpawnPools::default()
            },
        };
        self.pools.merge(pools);
    }
}
