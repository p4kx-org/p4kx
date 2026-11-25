// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::{ContractPiece, ObservedPiece, Piece, PieceError, project};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalRecordScope};
use crate::catalog_v3::mission_sources::contract_generator_facets::{
    self, AuthoredGeneratorFacets,
};
use crate::contract_generator_definition::{self, DefinitionProjection, OccurrenceFacts};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::mission_entry_source::MissionEntryContext;
use crate::mission_generator::GeneratorDefinition;
use crate::mission_index::demand::{DependencyKey, DependencyRequired};
use crate::record_observation::RecordObservation;
use crate::source_fields::{FieldCapture, FieldError};
use crate::source_value_catalog::{self, SourceArenaOutput};
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
    facets: GeneratorFacets,
) -> Result<ContractAttempt<'source>, CatalogError> {
    project_observed(input, admission, tree, context, role, cancel, facets)
}

/// A whole source transaction or the precise next canonical input it requires.
pub(crate) enum ContractAttempt<'source> {
    Complete(Box<ObservedPiece<'source>>),
    Dependency(DependencyKey),
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratorFacets {
    All,
    Contracts,
    Summaries,
}

impl GeneratorFacets {
    fn source<'scope, 'read, 'source>(
        self,
        scope: &'scope OriginalRecordScope<'read, 'source>,
        cancel: Option<&'scope CancelToken>,
    ) -> Result<GeneratorSource<'scope, 'read, 'source>, CatalogError> {
        match self {
            Self::Summaries => Ok(GeneratorSource {
                fields: OriginalFlowFields::read(scope, cancel)?,
                arena: None,
            }),
            Self::All | Self::Contracts => {
                let indexed = source_value_catalog::read_indexed_arena(scope, cancel, |_| {})?;
                Ok(GeneratorSource {
                    fields: indexed.facts,
                    arena: Some(indexed.source),
                })
            }
        }
    }
}

struct GeneratorSource<'scope, 'read, 'source> {
    fields: OriginalFlowFields<'scope, 'read, 'source>,
    arena: Option<SourceArenaOutput<'source>>,
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
    let GeneratorSource { fields, arena } = facets.source(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&fields, input, admission)?;
    let (definition, OccurrenceFacts { item_error, notes }) = if let Some(arena) = arena {
        let DefinitionProjection {
            definition,
            item_error,
            notes,
        } = contract_generator_definition::read(input, tree, &fields, arena, &capture)?;
        (Some(definition), OccurrenceFacts { item_error, notes })
    } else {
        (
            None,
            contract_generator_definition::occurrence_facts(&fields, &capture)?,
        )
    };
    let mut authored = authored_facets(facets, input, tree, &fields, &capture, notes)?;
    crate::source_projection_notes::record(
        Kind::Contracts,
        role,
        admission,
        tree,
        std::mem::take(&mut authored.notes),
        &mut ledger,
    )?;
    let projected =
        item_error.map_or_else(|| project(admission, tree, context, &fields, &capture), Err);
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
    definition: Option<GeneratorDefinition<'source>>,
    authored: AuthoredGeneratorFacets<'source>,
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
        definition,
        scenario_requirements: authored.scenario_requirements,
        local_generators: authored.local_generators,
        ledger,
    })
}

fn authored_facets<'source>(
    facets: GeneratorFacets,
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: Vec<FieldError>,
) -> Result<AuthoredGeneratorFacets<'source>, CatalogError> {
    match facets {
        GeneratorFacets::All => {
            contract_generator_facets::read(input, tree, fields, capture, notes)
        }
        GeneratorFacets::Contracts | GeneratorFacets::Summaries => Ok(AuthoredGeneratorFacets {
            notes,
            ..AuthoredGeneratorFacets::default()
        }),
    }
}
