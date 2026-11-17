// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::{ContractPiece, ObservedPiece, Piece, PieceError, project};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::mission_sources::{
    contract_scenario_requirements, local_contract_generator_configuration,
};
use crate::contract_generator_definition::{self, DefinitionProjection};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::mission_entry_source::MissionEntryContext;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_index::demand::{DependencyKey, DependencyRequired};
use crate::record_observation::RecordObservation;
use crate::source_fields::{FieldCapture, FieldError};
use crate::source_value_catalog;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure, Kind};

pub(crate) fn project_tree<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    context: &MissionEntryContext<'_, 'source>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<ObservedPiece<'source>, CatalogError> {
    match project_observed(
        input,
        admission,
        tree,
        context,
        role,
        cancel,
        GeneratorFacets::All,
    )? {
        ContractAttempt::Complete(observed) => Ok(*observed),
        ContractAttempt::Dependency(_) => {
            Err(crate::ExportAccountingError::ConflictingSourceDatum.into())
        }
    }
}

pub(crate) fn try_project_contracts_tree<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    context: &MissionEntryContext<'_, 'source>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<ContractAttempt<'source>, CatalogError> {
    project_observed(
        input,
        admission,
        tree,
        context,
        role,
        cancel,
        GeneratorFacets::Contracts,
    )
}

/// A whole source transaction or the precise next canonical input it requires.
pub(crate) enum ContractAttempt<'source> {
    Complete(Box<ObservedPiece<'source>>),
    Dependency(DependencyKey),
}

#[derive(Clone, Copy)]
enum GeneratorFacets {
    All,
    Contracts,
}

fn project_observed<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    context: &MissionEntryContext<'_, 'source>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
    facets: GeneratorFacets,
) -> Result<ContractAttempt<'source>, CatalogError> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        tree,
        admission,
        Some(Kind::Contracts),
        role,
        cancel,
    )?;
    let scope = input.record_scope(admission.record(), tree)?;
    let indexed = source_value_catalog::read_indexed_arena(&scope, cancel, |_| {})?;
    let fields = &indexed.facts;
    let capture = TemplateFieldCapture::new(fields, input, admission)?;
    let DefinitionProjection {
        definition,
        item_error,
        notes,
    } = contract_generator_definition::read(input, tree, fields, indexed.source, &capture)?;
    let mut authored = authored_facets(facets, input, tree, fields, &capture, notes)?;
    crate::source_projection_notes::record(
        Kind::Contracts,
        role,
        admission,
        tree,
        std::mem::take(&mut authored.notes),
        &mut ledger,
    )?;
    let projected =
        item_error.map_or_else(|| project(admission, tree, context, fields, &capture), Err);
    let projected = match projected {
        Err(error) => match error.downcast::<DependencyRequired>() {
            Ok(request) => return Ok(ContractAttempt::Dependency(request.key)),
            Err(error) => Err(error),
        },
        Ok(piece) => Ok(piece),
    };
    finish_observed(projected, admission, role, definition, authored, ledger)
        .map(|observed| ContractAttempt::Complete(Box::new(observed)))
}

fn finish_observed<'source>(
    projected: Result<ContractPiece<'source>, PieceError>,
    admission: &RecordObservation<'_, 'source>,
    role: ExportSourceRole,
    definition: GeneratorDefinition<'source>,
    authored: Authored<'source>,
    mut ledger: ExportLedger,
) -> Result<ObservedPiece<'source>, CatalogError> {
    let piece = match projected {
        Ok(piece) => {
            crate::mission_ledger::observe(
                &mut ledger,
                admission,
                role,
                SourceDisposition::Projected,
                None,
            )?;
            Piece::Contracts(Box::new(piece))
        }
        Err(error) => match error.downcast::<CatalogError>() {
            Ok(error) => return Err(*error),
            Err(error) => {
                crate::mission_ledger::observe(
                    &mut ledger,
                    admission,
                    role,
                    SourceDisposition::ProjectionFailed,
                    Some(error.as_ref()),
                )?;
                Piece::Failed(CatalogReadFailure::of(admission.record(), error.as_ref()))
            }
        },
    };
    Ok(ObservedPiece {
        piece,
        definition: Some(definition),
        scenario_requirements: authored.scenario_requirements,
        local_generators: authored.local_generators,
        ledger,
    })
}

struct Authored<'source> {
    scenario_requirements:
        Vec<contract_scenario_requirements::ContractScenarioRequirements<'source>>,
    local_generators:
        Vec<local_contract_generator_configuration::LocalContractGeneratorConfiguration<'source>>,
    notes: Vec<FieldError>,
}

fn authored<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    mut notes: Vec<FieldError>,
) -> Result<Authored<'source>, CatalogError> {
    let scenario_requirements =
        contract_scenario_requirements::read(input, tree, fields, capture, &mut notes)?;
    let local_generators =
        local_contract_generator_configuration::read(input, tree, fields, capture, &mut notes)?;
    Ok(Authored {
        scenario_requirements,
        local_generators,
        notes,
    })
}

fn authored_facets<'source>(
    facets: GeneratorFacets,
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: Vec<FieldError>,
) -> Result<Authored<'source>, CatalogError> {
    match facets {
        GeneratorFacets::All => authored(input, tree, fields, capture, notes),
        GeneratorFacets::Contracts => Ok(Authored {
            scenario_requirements: Vec::new(),
            local_generators: Vec::new(),
            notes,
        }),
    }
}
