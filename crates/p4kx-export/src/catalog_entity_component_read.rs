// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, PointerProbe, Record, RecordTree, TreeBuilder,
    Value,
};

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
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

/// Reads an independent component family from physical Entity owners admitted by the Record filter.
pub(super) fn read<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    kind: &ComponentKind,
    project: Projection<'source, Row>,
) -> Result<ComponentRead<'source, Row>, CatalogError> {
    input.check_forge(forge)?;
    let mut read = ComponentRead {
        rows: Vec::new(),
        numbers: Vec::new(),
        ledger: ExportLedger::default(),
        failures: Vec::new(),
    };
    let Some(probe) = forge.pointer_probe(COMPONENTS, kind.source_type) else {
        return Ok(read);
    };
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if record.type_name().ok() != Some(ENTITY) {
            continue;
        }
        match select(&mut trees, &probe, &record, filter) {
            Ok(false) => continue,
            Err(error) => {
                let admission = RecordObservation::capture(input, &record)?;
                read.failed(
                    &admission,
                    ExportSourceRole::SelectionCandidate,
                    kind,
                    &error,
                )?;
                continue;
            }
            Ok(true) => {}
        }
        let admission = RecordObservation::capture(input, &record)?;
        match trees.build(&record) {
            Ok(tree) => read.project(&tree, input, &admission, cancel, kind, project)?,
            Err(error) => read.failed(&admission, DEPENDENCY, kind, &error)?,
        }
    }
    Ok(read)
}

fn select<'source>(
    trees: &mut TreeBuilder<'source>,
    probe: &PointerProbe<'source>,
    record: &Record<'source>,
    filter: Option<&Filter>,
) -> Result<bool, DataForgeError> {
    let accepted = filter.map(|filter| record.record_path().map(|path| filter.matches(path)));
    if matches!(accepted, Some(Ok(false))) || !trees.holds(probe, record)? {
        return Ok(false);
    }
    accepted.unwrap_or(Ok(true))
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
