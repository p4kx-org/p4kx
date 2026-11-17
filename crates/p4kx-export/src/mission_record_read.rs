// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree, StructValue, TreeBuilder};
use rayon::prelude::*;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::mission_sources::contract_scenario_requirements::ContractScenarioRequirements;
use crate::catalog_v3::mission_sources::local_contract_generator_configuration::LocalContractGeneratorConfiguration;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::mission_entry_source::{MissionEntryContext, MissionEntrySource};
use crate::mission_generator::{ContractOwner, GeneratorDefinition};
use crate::mission_intros::{Grants, PendingContract};
use crate::mission_layers::{Layer, one};
use crate::mission_places::Where;
use crate::mission_property_set::MissionPropertySets;
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::mission_text_inputs::{LayerKind, LayerTexts};
use crate::record_observation::RecordObservation;
use crate::selection::Kind;
use crate::source_fields::FieldCapture;
use crate::spawn_layers::RecordSpawns;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogReadFailure};

/// A source Record that cannot be read or projected into contract rows.
type PieceError = Box<dyn std::error::Error + Send + Sync>;

pub(crate) struct ContractPiece<'f> {
    pub(crate) contracts: Vec<PendingContract<'f>>,
    pub(crate) skipped: usize,
    pub(crate) grants: Grants,
    pub(crate) pools: SpawnPools<'f>,
    pub(crate) property_sets: MissionPropertySets<'f>,
}

struct ContractSource<'t, 'f> {
    tree: &'t RecordTree<'f>,
    spawns: &'t RecordSpawns<'t, 'f>,
    pub(crate) property_sets: &'t MissionPropertySetRegistry<'t, 'f>,
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
        crate::mission_generator::entries::candidates(
            self.tree,
            handler,
            fields,
            grants,
            |candidate| {
                let parent = MissionEntrySource {
                    tree: self.tree,
                    handler,
                    handler_layer: &handler_layer,
                    handler_texts: &handler_texts,
                    handler_where: &handler_where,
                    spawns: &handler_spawns,
                    contract: candidate.contract,
                    name: candidate.name,
                    property_sets: self.property_sets,
                };
                let owner =
                    ContractOwner::read(fields, handler, candidate.container, candidate.contract)?;
                parent.entries(context, &owner, contracts)
            },
        )
    }
}
pub(crate) enum Piece<'f> {
    Contracts(Box<ContractPiece<'f>>),
    Failed(CatalogReadFailure<'f>),
}

pub(crate) struct ObservedPiece<'f> {
    pub(crate) piece: Piece<'f>,
    pub(crate) definition: Option<GeneratorDefinition<'f>>,
    pub(crate) scenario_requirements: Vec<ContractScenarioRequirements<'f>>,
    pub(crate) local_generators: Vec<LocalContractGeneratorConfiguration<'f>>,
    pub(crate) ledger: ExportLedger,
}

/// Reads each of `records` in parallel; a Record that cannot be read is a failure.
pub(super) fn read<'f>(
    records: &[Record<'f>],
    input: &OriginalCatalogInput<'_, 'f>,
    context: &MissionEntryContext<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ObservedPiece<'f>>, CatalogError> {
    records
        .par_iter()
        .map_init(TreeBuilder::new, |trees, record| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            piece(trees, record, input, context, cancel)
        })
        .filter_map(Result::transpose)
        .collect()
}

fn piece<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    input: &OriginalCatalogInput<'_, 'f>,
    context: &MissionEntryContext<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Option<ObservedPiece<'f>>, CatalogError> {
    let rule = Kind::Contracts.records();
    let admission = RecordObservation::capture(input, record)?;
    match record.type_name() {
        Ok(kind) if kind != rule.type_name => return Ok(None),
        Err(error) => {
            return Ok(Some(failed(
                &admission,
                &error,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
                ExportLedger::default(),
            )?));
        }
        _ => {}
    }
    let tree = match trees.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            return Ok(Some(failed(
                &admission,
                &error,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
                ExportLedger::default(),
            )?));
        }
    };
    project_tree(
        input,
        &admission,
        &tree,
        context,
        ExportSourceRole::Selected,
        cancel,
    )
    .map(Some)
}

#[path = "mission_record_projection.rs"]
mod projection;
pub(crate) use projection::{ContractAttempt, project_tree, try_project_contracts_tree};

fn failed<'f>(
    admission: &RecordObservation<'_, 'f>,
    error: &(dyn std::error::Error + 'static),
    role: ExportSourceRole,
    disposition: SourceDisposition,
    mut ledger: ExportLedger,
) -> Result<ObservedPiece<'f>, CatalogError> {
    crate::mission_ledger::observe(&mut ledger, admission, role, disposition, Some(error))?;
    Ok(ObservedPiece {
        piece: Piece::Failed(CatalogReadFailure::of(admission.record(), error)),
        definition: None,
        scenario_requirements: Vec::new(),
        local_generators: Vec::new(),
        ledger,
    })
}

fn project<'f>(
    admission: &RecordObservation<'_, 'f>,
    tree: &RecordTree<'f>,
    context: &MissionEntryContext<'_, 'f>,
    fields: &OriginalFlowFields<'_, '_, 'f>,
    capture: &dyn FieldCapture<'f>,
) -> Result<ContractPiece<'f>, PieceError> {
    let mut contracts = Vec::new();
    let mut grants = Grants::new(context.projection.origins());
    let spawns = RecordSpawns::new(context.index.tags(), tree, capture)?;
    let property_sets =
        MissionPropertySetRegistry::new_observed(admission, tree, context.index.tags(), capture)?;
    let source = ContractSource {
        tree,
        spawns: &spawns,
        property_sets: &property_sets,
    };
    let record = context
        .projection
        .original()
        .record_context(admission.record())?;
    let skipped = crate::mission_generator::entries::handlers(
        tree,
        fields,
        &record,
        &mut grants,
        |handler, grants| source.handler(handler, context, fields, &mut contracts, grants),
    )?;
    let (property_sets, descriptions) = property_sets.finish();
    let mut pools = spawns.into_pools();
    pools.merge(descriptions);
    Ok(ContractPiece {
        contracts,
        skipped,
        grants,
        pools,
        property_sets,
    })
}
