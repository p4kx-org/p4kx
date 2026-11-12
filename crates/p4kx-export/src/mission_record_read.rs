// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree, TreeBuilder};
use rayon::prelude::*;

use super::{ContractPiece, ContractSource, Grants, ObservedPiece, Piece, PieceError};
use crate::blueprint;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::mission_sources::{
    contract_scenario_requirements, local_contract_generator_configuration,
};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::mission_entry_source::MissionEntryContext;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_layers;
use crate::mission_property_set_registry::MissionPropertySetRegistry;
use crate::record_observation::RecordObservation;
use crate::selection::Kind;
use crate::source_fields::FieldCapture;
use crate::source_value_catalog;
use crate::spawn_layers::RecordSpawns;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure};
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
    project_tree(input, &admission, &tree, context, cancel).map(Some)
}

fn project_tree<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    context: &MissionEntryContext<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ObservedPiece<'source>, CatalogError> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        tree,
        admission,
        Some(Kind::Contracts),
        ExportSourceRole::Selected,
        cancel,
    )?;
    let scope = input.record_scope(admission.record(), tree)?;
    let indexed = source_value_catalog::read_indexed_arena(&scope, cancel, |_| {})?;
    let fields = &indexed.facts;
    let definition = GeneratorDefinition {
        input_ref: fields.input_ref().clone(),
        header: CanonicalRecordHeader::read(tree, input.origins())?,
        source: indexed.source,
    };
    let capture = TemplateFieldCapture::new(fields, input, admission)?;
    let Authored {
        scenario_requirements,
        local_generators,
    } = authored(input, admission, tree, fields, &capture, &mut ledger)?;
    match project(admission, tree, context, fields, &capture) {
        Ok(piece) => {
            crate::mission_ledger::observe(
                &mut ledger,
                admission,
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
                None,
            )?;
            Ok(ObservedPiece {
                piece: Piece::Contracts(Box::new(piece)),
                definition: Some(definition),
                scenario_requirements,
                local_generators,
                ledger,
            })
        }
        Err(error) => match error.downcast::<CatalogError>() {
            Ok(error) => Err(*error),
            Err(error) => {
                let mut observed = failed(
                    admission,
                    error.as_ref(),
                    ExportSourceRole::Selected,
                    SourceDisposition::ProjectionFailed,
                    ledger,
                )?;
                observed.definition = Some(definition);
                observed.scenario_requirements = scenario_requirements;
                observed.local_generators = local_generators;
                Ok(observed)
            }
        },
    }
}

struct Authored<'source> {
    scenario_requirements:
        Vec<contract_scenario_requirements::ContractScenarioRequirements<'source>>,
    local_generators:
        Vec<local_contract_generator_configuration::LocalContractGeneratorConfiguration<'source>>,
}

fn authored<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    ledger: &mut ExportLedger,
) -> Result<Authored<'source>, CatalogError> {
    let mut notes = Vec::new();
    let scenario_requirements =
        contract_scenario_requirements::read(input, tree, fields, capture, &mut notes)?;
    let local_generators =
        local_contract_generator_configuration::read(input, tree, fields, capture, &mut notes)?;
    crate::source_projection_notes::record(
        Kind::Contracts,
        ExportSourceRole::Selected,
        admission,
        tree,
        notes,
        ledger,
    )?;
    Ok(Authored {
        scenario_requirements,
        local_generators,
    })
}

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
    let mut skipped = 0_usize;
    let mut grants = Grants::new(context.projection.origins());
    let spawns = RecordSpawns::new(context.index.tags(), tree, capture)?;
    let property_sets = MissionPropertySetRegistry::new_observed(
        admission,
        tree,
        context.index.tags(),
        context.texts,
        capture,
    )?;
    let source = ContractSource {
        tree,
        spawns: &spawns,
        property_sets: &property_sets,
    };
    let record = context
        .projection
        .original()
        .record_context(admission.record())?;
    fields.check_tree(tree)?;
    grants.begin_record(fields, &record)?;
    for value in mission_layers::items(Some(tree.root()), "generators") {
        grants.handler_slot(fields, value)?;
        if let Some(handler) = blueprint::as_struct(value) {
            skipped = skipped.saturating_add(source.handler(
                handler,
                context,
                fields,
                &mut contracts,
                &mut grants,
            )?);
        }
    }
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
