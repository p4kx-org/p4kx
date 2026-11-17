// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, SourceUseRole};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::mission_sources::contract_scenario_requirements::ContractScenarioRequirements;
use crate::catalog_v3::mission_sources::local_contract_generator_configuration::LocalContractGeneratorConfiguration;
use crate::catalog_v3::tag_catalog::{CapturedTags, TagDependencies};
use crate::contract_missions::ContractWithTitles;
use crate::contract_starmap::StarmapObject;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::mission_entry_source::MissionEntryContext;
use crate::mission_index::MissionIndex;
use crate::mission_intros::{self, Grants, PendingContract};
use crate::mission_places::Places;
use crate::mission_property_set::MissionPropertySets;
use crate::mission_siblings::{self, Siblings};
use crate::options::CatalogStage::ProjectContractGenerators;
use crate::options::{CatalogRun, CatalogStage};
use crate::selection::Kind;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogReadFailure};

#[path = "mission_record_read.rs"]
pub(crate) mod record_read;
use self::record_read::{ContractPiece, ObservedPiece, Piece};

#[path = "mission_property_set_collision.rs"]
mod property_set_collision;

/// The contracts of a DataForge and what was left out.
pub(crate) struct Missions<'f> {
    pub(crate) ledger: ExportLedger,
    pub(crate) contracts: Vec<ContractWithTitles<'f>>,
    pub(crate) generators: StagedCatalogTable,
    pub(crate) scenario_requirements: Vec<ContractScenarioRequirements<'f>>,
    pub(crate) local_generators: Vec<LocalContractGeneratorConfiguration<'f>>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) pools: SpawnPools<'f>,
    pub(crate) siblings: Siblings<'f>,
    pub(crate) property_sets: MissionPropertySets<'f>,
}

/// Reads the contracts of the `ContractGenerator` Records `filter` matches: for each contract an
/// entry, and one for each of its sub contracts, ordered by the text of the GUIDs. The pickup
/// places are the starmap objects `starmap` lists.
///
/// A contract or sub contract whose debug name or Record is a development leftover, one without an
/// `id`, and an entry whose `id` pair an earlier entry already has, is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    projection: &CatalogProjection<'_, 'f>,
    (tags, dependencies): (&CapturedTags<'_, 'f>, Option<TagDependencies<'f>>),
    starmap: &[StarmapObject<'f>],
    run: CatalogRun<'_, '_>,
) -> Result<Missions<'f>, CatalogError> {
    let filter = run.options.filter;
    let cancel = run.options.cancel;
    let input = projection.original();
    let uses = &input.origins().uses;
    let mut grants = Grants::new(projection.origins());
    let records = selected_generators(forge, input, filter, cancel, &mut grants)?;
    run.begin(CatalogStage::ReadMissionDependencies);
    let mut index = MissionIndex::read_captured(
        forge,
        input,
        filter,
        cancel,
        dependencies,
        tags,
        forge.records(),
    )?;
    let (pools, failures, mut ledger) = index.take_template_results();
    run.begin(CatalogStage::ProjectMissionDefinitions);
    let siblings = mission_siblings::read(
        forge,
        projection,
        (tags, &index),
        run,
        &mut ledger.field_inspections,
    )?;
    let generator_type = Kind::Contracts.records().type_name;
    let availability =
        DatasetAvailability::from_source_declarations(forge, |name| name == generator_type);
    let mut missions = Missions::empty(availability, pools, failures, ledger, siblings);
    crate::mission_ledger::inventory(&mut missions.ledger);
    run.begin_compacted(ProjectContractGenerators, uses, &missions.ledger)?;
    if records.is_empty() {
        return Ok(missions);
    }
    let places = Places::new(starmap);
    let contract_projection = projection.for_kind(Kind::Contracts);
    let context = MissionEntryContext {
        index: &index,
        projection: &contract_projection,
        places: &places,
        cancel,
    };
    let scratch = std::env::temp_dir();
    let mut generators = StagedCatalogRows::new(&scratch, cancel);
    let mut pending = Vec::new();
    for window in records.chunks(rayon::current_num_threads()) {
        for piece in record_read::read(window, input, &context, cancel)? {
            missions.take(piece, &mut pending, &mut grants, &mut generators)?;
        }
        uses.compact_record_uses(|| run.check_cancelled())?;
    }
    drop(index);
    missions.generators = generators.finish(availability)?;
    missions.complete_contracts(pending, grants)?;
    Ok(missions)
}

fn selected_generators<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, '_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    grants: &mut Grants,
) -> Result<Vec<Record<'source>>, CatalogError> {
    let rule = Kind::Contracts.records();
    let mut selected = Vec::new();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let Ok(path) = record.record_path() else {
            let source_type = match record.type_name() {
                Ok(kind) if kind == rule.type_name => SourceUseRole::Selector,
                Err(_) => SourceUseRole::Attempted,
                Ok(_) => continue,
            };
            let context = input.record_context(&record)?;
            grants.unreadable_path(&context, source_type)?;
            continue;
        };
        if crate::contract_generator_definition::matches_path(path, filter) {
            selected.push(record);
        }
    }
    Ok(selected)
}

impl<'f> Missions<'f> {
    fn empty(
        availability: DatasetAvailability,
        pools: SpawnPools<'f>,
        failures: Vec<CatalogReadFailure<'f>>,
        ledger: ExportLedger,
        siblings: Siblings<'f>,
    ) -> Self {
        Self {
            ledger,
            contracts: Vec::new(),
            generators: StagedCatalogTable::empty(availability),
            scenario_requirements: Vec::new(),
            local_generators: Vec::new(),
            skipped: 0,
            failures,
            pools,
            siblings,
            property_sets: MissionPropertySets::default(),
        }
    }

    fn complete_contracts(
        &mut self,
        contracts: Vec<PendingContract<'f>>,
        grants: Grants,
    ) -> Result<(), CatalogError> {
        let completed =
            finalize_contracts(contracts, &self.property_sets, grants, &mut self.ledger)?;
        self.skipped = self.skipped.saturating_add(completed.duplicates);
        self.ledger.skipped_rows(Kind::Contracts, self.skipped);
        self.contracts = completed.rows;
        Ok(())
    }
    fn take(
        &mut self,
        observed: ObservedPiece<'f>,
        pending: &mut Vec<PendingContract<'f>>,
        grants: &mut Grants,
        generators: &mut StagedCatalogRows<'_>,
    ) -> Result<(), CatalogError> {
        self.ledger.merge(observed.ledger);
        self.scenario_requirements
            .extend(observed.scenario_requirements);
        self.local_generators.extend(observed.local_generators);
        if let Some(definition) = observed.definition {
            generators.append(&definition)?;
        }
        match observed.piece {
            Piece::Contracts(piece) => {
                let ContractPiece {
                    contracts,
                    skipped,
                    grants: granted,
                    pools: found,
                    property_sets,
                } = *piece;
                self.property_sets.merge(property_sets)?;
                self.pools.merge(found);
                pending.extend(contracts);
                self.skipped = self.skipped.saturating_add(skipped);
                grants.merge(granted)?;
            }
            Piece::Failed(failure) => {
                self.failures.push(failure);
                grants.incomplete();
            }
        }
        Ok(())
    }
}

/// Canonical rows and the logical duplicates removed by finalization.
pub(crate) struct FinalizedContracts<'source> {
    pub(crate) rows: Vec<ContractWithTitles<'source>>,
    pub(crate) duplicates: usize,
}

/// Resolves shared property identities, canonical duplicate order and intro dependencies.
pub(crate) fn finalize_contracts<'source>(
    contracts: Vec<PendingContract<'source>>,
    sets: &MissionPropertySets<'source>,
    grants: Grants,
    ledger: &mut ExportLedger,
) -> Result<FinalizedContracts<'source>, CatalogError> {
    finalize_contracts_scoped(contracts, sets, grants, ledger, |_| true)
}

pub(crate) fn finalize_contracts_scoped<'source>(
    mut contracts: Vec<PendingContract<'source>>,
    sets: &MissionPropertySets<'source>,
    grants: Grants,
    ledger: &mut ExportLedger,
    mut retain: impl FnMut(&PendingContract<'source>) -> bool,
) -> Result<FinalizedContracts<'source>, CatalogError> {
    property_set_collision::finalize(
        sets,
        contracts.iter_mut().map(|contract| &mut contract.base),
        ledger,
    )?;
    let duplicates = order(&mut contracts);
    contracts.retain(|row| retain(row));
    Ok(FinalizedContracts {
        rows: mission_intros::resolve(contracts, grants)?,
        duplicates,
    })
}

/// Orders rows by GUID and counts duplicates removed by the logical ID pair.
fn order(contracts: &mut Vec<PendingContract<'_>>) -> usize {
    contracts.sort_by_cached_key(|contract| {
        let entry = &contract.base;
        (
            entry.guid.value().copied().map(CatalogGuid::text),
            entry
                .sub_contract_guid
                .value()
                .copied()
                .map(CatalogGuid::text),
            entry.record_guid.text(),
        )
    });
    let listed = contracts.len();
    contracts.dedup_by(|later, earlier| {
        later.base.guid.value() == earlier.base.guid.value()
            && later.base.sub_contract_guid.value() == earlier.base.sub_contract_guid.value()
    });
    listed.saturating_sub(contracts.len())
}
