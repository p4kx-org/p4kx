// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashSet;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, PointerProbe, Record, RecordTree, TreeBuilder,
    Value,
};

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::OriginalFlowFields;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_field_walk;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::export_source_failure;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::source_projection_notes;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENTS: &str = "Components";
const DEPENDENCY: ExportSourceRole = ExportSourceRole::Dependency;

pub(super) struct ComponentRead<'source, Row> {
    pub(super) rows: Vec<Row>,
    pub(super) numbers: Vec<CapturedNumber>,
    pub(super) ledger: ExportLedger,
    pub(super) failures: Vec<CatalogReadFailure<'source>>,
}

#[derive(Clone, Copy)]
pub(super) struct ComponentKind {
    pub(super) source_type: &'static str,
    pub(super) inspection: ConfigurationInspection,
}

pub(super) type Projection<'source, Row> = fn(
    ComponentOwner,
    Observed<'_, '_, '_, 'source>,
    &mut Vec<FieldError>,
) -> Result<Row, CatalogError>;

pub(super) struct ComponentSelection<'a> {
    filter: Option<&'a Filter>,
    dependencies: Option<&'a HashSet<u32>>,
    unclassified_role: Option<ExportSourceRole>,
    physical: CatalogRecordSelection,
}

impl<'a> ComponentSelection<'a> {
    pub(super) const fn export(
        filter: Option<&'a Filter>,
        dependencies: Option<&'a HashSet<u32>>,
    ) -> Self {
        Self {
            filter,
            dependencies,
            unclassified_role: None,
            physical: CatalogRecordSelection::All,
        }
    }

    pub(super) const fn discovery() -> Self {
        Self {
            filter: None,
            dependencies: None,
            unclassified_role: Some(ExportSourceRole::SelectionCandidate),
            physical: CatalogRecordSelection::All,
        }
    }
}

/// Reads an independent component family from selected and linked physical Entity owners.
pub(super) fn read<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    selection: &ComponentSelection<'_>,
    cancel: Option<&CancelToken>,
    kind: &ComponentKind,
    project: Projection<'source, Row>,
) -> Result<ComponentRead<'source, Row>, CatalogError> {
    read_in(forge, input, selection, cancel, kind, project, None)
}

pub(super) fn read_selected<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    owners: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    kind: &ComponentKind,
    project: Projection<'source, Row>,
) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
    checkpoint(cancel)?;
    let selection = ComponentSelection {
        physical: owners,
        ..ComponentSelection::discovery()
    };
    let mut coverage = CatalogRecordCoverage::default();
    let read = read_in(
        forge,
        input,
        &selection,
        cancel,
        kind,
        project,
        Some(&mut coverage),
    )?;
    Ok((read.rows, coverage))
}

/// Reads one physical owner with strict coverage and retains canonical partial rows.
pub(super) fn read_selected_with_partials<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    record_index: u32,
    cancel: Option<&CancelToken>,
    kind: &ComponentKind,
    project: Projection<'source, Row>,
) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
    let physical = CatalogRecordSelection::RecordIndex(record_index);
    let (rows, coverage) = read_selected(forge, input, physical, cancel, kind, project)?;
    if !rows.is_empty() || coverage.unreadable_bodies.is_empty() {
        return Ok((rows, coverage));
    }
    checkpoint(cancel)?;
    let selection = ComponentSelection {
        physical,
        ..ComponentSelection::export(None, None)
    };
    let canonical = read(forge, input, &selection, cancel, kind, project)?;
    Ok((canonical.rows, coverage))
}

fn read_in<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    selection: &ComponentSelection<'_>,
    cancel: Option<&CancelToken>,
    kind: &ComponentKind,
    project: Projection<'source, Row>,
    mut coverage: Option<&mut CatalogRecordCoverage>,
) -> Result<ComponentRead<'source, Row>, CatalogError> {
    input.check_forge(forge)?;
    let mut read = ComponentRead {
        rows: Vec::new(),
        numbers: Vec::new(),
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
    let probe = if selection.unclassified_role.is_some() {
        forge.strict_pointer_probe(COMPONENTS, kind.source_type)
    } else {
        forge.pointer_probe(COMPONENTS, kind.source_type)
    };
    let Some(probe) = probe else {
        return Ok(read);
    };
    let mut trees = TreeBuilder::new();
    selection.physical.for_each(forge, cancel, |record| {
        match candidate(&mut trees, &probe, record, selection) {
            Ok(false) => return Ok(()),
            Err(failure) => {
                if let Some(role) = failure.role {
                    let admission = RecordObservation::capture(input, record)?;
                    read.failed(&admission, role, kind, &failure.error)?;
                }
                if let Some(coverage) = coverage.as_deref_mut() {
                    failure.stage.cover(coverage, record.index());
                }
                return Ok(());
            }
            Ok(true) => {}
        }
        let admission = RecordObservation::capture(input, record)?;
        match trees.build(record) {
            Ok(tree) => {
                let inspections = read.ledger.field_inspections.len();
                let diagnostics = read.ledger.field_diagnostics.len();
                read.project(&tree, input, &admission, cancel, kind, project)?;
                if let Some(coverage) = coverage.as_deref_mut()
                    && !read
                        .ledger
                        .field_projection_complete(inspections, diagnostics)
                {
                    coverage.incomplete_fields.push(record.index());
                }
            }
            Err(error) => {
                read.failed(&admission, DEPENDENCY, kind, &error)?;
                if let Some(coverage) = coverage.as_deref_mut() {
                    FailureStage::for_failed_tree(record).cover(coverage, record.index());
                }
            }
        }
        Ok(())
    })?;
    Ok(read)
}

enum FailureStage {
    Header,
    Body,
}

impl FailureStage {
    fn for_failed_tree(record: &Record<'_>) -> Self {
        if record.stored_file_name().is_err()
            || record.stored_name().is_err()
            || record.stored_tag().is_err()
        {
            Self::Header
        } else {
            Self::Body
        }
    }

    fn cover(self, coverage: &mut CatalogRecordCoverage, index: u32) {
        match self {
            Self::Header => coverage.unreadable_headers.push(index),
            Self::Body => coverage.unreadable_bodies.push(index),
        }
    }
}

struct CandidateFailure {
    stage: FailureStage,
    role: Option<ExportSourceRole>,
    error: DataForgeError,
}

fn candidate<'source>(
    trees: &mut TreeBuilder<'source>,
    probe: &PointerProbe<'source>,
    record: &Record<'source>,
    selection: &ComponentSelection<'_>,
) -> Result<bool, CandidateFailure> {
    let source_type = record.type_name().map_err(|error| CandidateFailure {
        stage: FailureStage::Header,
        role: selection.unclassified_role,
        error,
    })?;
    if source_type != ENTITY {
        return Ok(false);
    }
    select(trees, probe, record, selection)
}

fn select<'source>(
    trees: &mut TreeBuilder<'source>,
    probe: &PointerProbe<'source>,
    record: &Record<'source>,
    selection: &ComponentSelection<'_>,
) -> Result<bool, CandidateFailure> {
    let accepted = if selection
        .dependencies
        .is_some_and(|indices| indices.contains(&record.index()))
    {
        None
    } else {
        selection
            .filter
            .map(|filter| record.record_path().map(|path| filter.matches(path)))
    };
    let role = Some(ExportSourceRole::SelectionCandidate);
    if matches!(accepted, Some(Ok(false)))
        || !trees
            .holds(probe, record)
            .map_err(|error| CandidateFailure {
                stage: FailureStage::Body,
                role,
                error,
            })?
    {
        return Ok(false);
    }
    accepted
        .unwrap_or(Ok(true))
        .map_err(|error| CandidateFailure {
            stage: FailureStage::Header,
            role,
            error,
        })
}

impl<'source, Row> ComponentRead<'source, Row> {
    fn project(
        &mut self,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
        kind: &ComponentKind,
        project: Projection<'source, Row>,
    ) -> Result<(), CatalogError> {
        let Some(Value::Array(components)) = tree.field(COMPONENTS) else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let scope = input.record_scope(admission.record(), tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let capture = TemplateFieldCapture::new(&originals, input, admission)?;
        let context = input
            .origins()
            .record(tree.record_index())
            .with_cancel(cancel);
        let numbers = RefCell::new(Vec::new());
        let observed = Observed::configuration(&originals, input, &capture, &numbers);
        let mut notes = Vec::new();
        let mut spans = Vec::new();
        for component in components.iter().filter(
            |component| matches!(component, Value::Struct(value) if value.type_name() == kind.source_type),
        ) {
            originals.check_cancel()?;
            let component = originals.fact(component)?;
            let owner = ComponentOwner::read(tree, &context, &originals, component)?;
            self.rows.push(project(
                owner,
                observed.at_owner(component),
                &mut notes,
            )?);
            spans.push(originals.span(component)?);
        }
        if spans.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.numbers.extend(numbers.into_inner());
        self.ledger.merge(export_field_walk::inspect_configuration(
            input,
            tree,
            admission,
            &spans,
            kind.inspection,
            cancel,
        )?);
        source_projection_notes::record_dependency(admission, tree, notes, &mut self.ledger)?;
        self.ledger.observe(
            admission
                .source(None, DEPENDENCY, SourceDisposition::Projected)?
                .for_configuration(kind.inspection),
        );
        Ok(())
    }

    fn failed(
        &mut self,
        admission: &RecordObservation<'_, 'source>,
        role: ExportSourceRole,
        kind: &ComponentKind,
        error: &DataForgeError,
    ) -> Result<(), CatalogError> {
        self.ledger.observe(
            admission
                .source(None, role, SourceDisposition::ReadFailed)?
                .for_configuration(kind.inspection)
                .diagnosed(export_source_failure::diagnosis(error)),
        );
        self.failures
            .push(CatalogReadFailure::of(admission.record(), error));
        Ok(())
    }
}

#[path = "catalog_thruster.rs"]
pub(super) mod catalog_thruster;

#[path = "catalog_flight_controller.rs"]
pub(super) mod catalog_flight_controller;

#[path = "catalog_ifcs.rs"]
pub(super) mod catalog_ifcs;
