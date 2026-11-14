// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Record, RecordTree, SourceUseRole, StructValue, Value,
};

use crate::blueprint::as_struct;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::mission_sources::contract_scenario_requirements::ContractScenarioRequirements;
use crate::catalog_v3::mission_sources::local_contract_generator_configuration::LocalContractGeneratorConfiguration;
use crate::catalog_v3::tag_catalog::{CapturedTags, TagDependencies};
use crate::contract_missions::ContractWithTitles;
use crate::contract_starmap::StarmapObject;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::mission_entry_source::{MissionEntryContext, MissionEntrySource};
use crate::mission_generator::{ContractOwner, GeneratorDefinition};
use crate::mission_index::MissionIndex;
use crate::mission_intros::{self, Candidates, Grants, INTRO_CONTRACTS, PendingContract};
use crate::mission_layers::{Layer, items, one};
use crate::mission_places::{Places, Where};
use crate::mission_property_set::MissionPropertySets;
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::mission_siblings::{self, Siblings};
use crate::mission_text_inputs::{LayerKind, LayerTexts};
use crate::record_fields::{identity, text};
use crate::selection::{Kind, is_leftover};
use crate::spawn_layers::RecordSpawns;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogReadFailure};

#[path = "mission_record_read.rs"]
mod record_read;

#[path = "mission_property_set_collision.rs"]
mod property_set_collision;

const CONTAINERS: [&str; 4] = [
    "contracts",
    INTRO_CONTRACTS,
    "legacyContracts",
    "PVPBountyContract",
];

/// The contracts of a DataForge and what was left out.
pub(crate) struct Missions<'f> {
    pub(crate) ledger: ExportLedger,
    pub(crate) contracts: Vec<ContractWithTitles<'f>>,
    pub(crate) generators: Vec<GeneratorDefinition<'f>>,
    pub(crate) scenario_requirements: Vec<ContractScenarioRequirements<'f>>,
    pub(crate) local_generators: Vec<LocalContractGeneratorConfiguration<'f>>,
    pub(crate) skipped: usize,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) pools: SpawnPools<'f>,
    pub(crate) siblings: Siblings<'f>,
    pub(crate) property_sets: MissionPropertySets<'f>,
}

/// A source Record that cannot be read or projected into contract rows.
type PieceError = Box<dyn std::error::Error + Send + Sync>;

struct ContractPiece<'f> {
    contracts: Vec<PendingContract<'f>>,
    skipped: usize,
    grants: Grants,
    pools: SpawnPools<'f>,
    property_sets: MissionPropertySets<'f>,
}

struct ContractSource<'t, 'f> {
    tree: &'t RecordTree<'f>,
    spawns: &'t RecordSpawns<'t, 'f>,
    property_sets: &'t MissionPropertySetRegistry<'t, 'f>,
}

impl<'t, 'f> ContractSource<'t, 'f> {
    fn handler(
        &self,
        handler: &'t StructValue<'f>,
        context: &MissionEntryContext<'_, 'f>,
        fields: &OriginalFlowFields<'_, '_, 'f>,
        contracts: &mut Vec<PendingContract<'f>>,
        grants: &mut Grants,
    ) -> Result<usize, PieceError> {
        let handler_layer = Layer::read(one(Some(handler), "contractParams"));
        let handler_texts = LayerTexts::read(
            fields,
            context.projection.origins(),
            handler,
            LayerKind::Handler,
        )?;
        let handler_where = Where::read(one(Some(handler), "defaultAvailability"), "prerequisites");
        let handler_spawns = self.spawns.handler(handler)?;
        let mut skipped = 0_usize;
        for container in CONTAINERS {
            let intro = (container == INTRO_CONTRACTS)
                .then(|| grants.candidates(fields, handler))
                .transpose()?;
            for value in items(Some(handler), container) {
                let Some(Candidate {
                    contract,
                    name,
                    admitted,
                }) = self.candidate(fields, grants, intro.as_ref(), value)?
                else {
                    continue;
                };
                if !admitted {
                    skipped = skipped.saturating_add(1);
                    continue;
                }
                let parent = MissionEntrySource {
                    tree: self.tree,
                    handler,
                    handler_layer: &handler_layer,
                    handler_texts: &handler_texts,
                    handler_where: &handler_where,
                    spawns: &handler_spawns,
                    contract,
                    name,
                    property_sets: self.property_sets,
                };
                let owner = ContractOwner::read(fields, handler, container, contract)?;
                skipped = skipped.saturating_add(parent.entries(context, &owner, contracts)?);
            }
        }
        Ok(skipped)
    }

    fn candidate<'scope, 'read>(
        &self,
        fields: &OriginalFlowFields<'scope, 'read, 'f>,
        grants: &mut Grants,
        intro: Option<&Candidates<'_, 'scope, 'read, 'f>>,
        value: &'t Value<'f>,
    ) -> Result<Option<Candidate<'t, 'f>>, CatalogError> {
        let contract = as_struct(value);
        let name = text(contract, "debugName");
        let id = contract
            .and_then(identity)
            .filter(|_| !is_leftover(self.tree.record_path(), name.unwrap_or_default()));
        if let Some(intro) = intro {
            grants.candidate(fields, intro, value, id)?;
        }
        Ok(contract.map(|contract| Candidate {
            contract,
            name,
            admitted: id.is_some(),
        }))
    }
}

struct Candidate<'read, 'source> {
    contract: &'read StructValue<'source>,
    name: Option<&'source str>,
    admitted: bool,
}

enum Piece<'f> {
    Contracts(Box<ContractPiece<'f>>),
    Failed(CatalogReadFailure<'f>),
}

struct ObservedPiece<'f> {
    piece: Piece<'f>,
    definition: Option<GeneratorDefinition<'f>>,
    scenario_requirements: Vec<ContractScenarioRequirements<'f>>,
    local_generators: Vec<LocalContractGeneratorConfiguration<'f>>,
    ledger: ExportLedger,
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
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Missions<'f>, CatalogError> {
    let input = projection.original();
    let texts = projection.legacy_texts();
    let mut grants = Grants::new(projection.origins());
    let records = selected_generators(forge, input, filter, cancel, &mut grants)?;
    let mut index = MissionIndex::read_captured(forge, input, filter, cancel, dependencies, tags)?;
    let (pools, failures, ledger) = index.take_template_results();
    let mut missions = Missions {
        ledger,
        contracts: Vec::new(),
        generators: Vec::new(),
        scenario_requirements: Vec::new(),
        local_generators: Vec::new(),
        skipped: 0,
        failures,
        pools,
        siblings: mission_siblings::read(forge, projection, (tags, &index), filter, cancel)?,
        property_sets: MissionPropertySets::default(),
    };
    crate::mission_ledger::inventory(&mut missions.ledger);
    if records.is_empty() {
        return Ok(missions);
    }
    let places = Places::new(starmap);
    let context = MissionEntryContext {
        index: &index,
        texts,
        projection,
        places: &places,
        cancel,
    };
    let observed = record_read::read(&records, input, &context, cancel)?;
    missions.complete_contracts(observed, grants)?;
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
        if rule.matches_path(path) && filter.is_none_or(|filter| filter.matches(path)) {
            selected.push(record);
        }
    }
    Ok(selected)
}

impl<'f> Missions<'f> {
    fn complete_contracts(
        &mut self,
        observed: Vec<ObservedPiece<'f>>,
        mut grants: Grants,
    ) -> Result<(), CatalogError> {
        let mut contracts = Vec::new();
        for piece in observed {
            self.take(piece, &mut contracts, &mut grants)?;
        }
        property_set_collision::finalize(
            &self.property_sets,
            contracts.iter_mut().map(|contract| &mut contract.base),
            &mut self.ledger,
        )?;
        self.skipped = self.skipped.saturating_add(order(&mut contracts));
        self.ledger.skipped_rows(Kind::Contracts, self.skipped);
        self.contracts = mission_intros::resolve(contracts, grants)?;
        Ok(())
    }

    fn take(
        &mut self,
        observed: ObservedPiece<'f>,
        pending: &mut Vec<PendingContract<'f>>,
        grants: &mut Grants,
    ) -> Result<(), CatalogError> {
        self.ledger.merge(observed.ledger);
        self.scenario_requirements
            .extend(observed.scenario_requirements);
        self.local_generators.extend(observed.local_generators);
        if let Some(definition) = observed.definition {
            self.generators.push(definition);
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
