// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, PointerProbe, Record, RecordTree, TreeBuilder,
    Value,
};

use super::authored_fields::Observed;
use super::catalog_mission_board::{COMPONENT, MissionBoard};
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
use crate::source_projection_notes;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENTS: &str = "Components";
const DEPENDENCY: ExportSourceRole = ExportSourceRole::Dependency;

#[derive(Default)]
pub(super) struct MissionBoardRead<'source> {
    pub(super) rows: Vec<MissionBoard<'source>>,
    pub(super) numbers: Vec<CapturedNumber>,
    pub(super) ledger: ExportLedger,
    pub(super) failures: Vec<CatalogReadFailure<'source>>,
}

/// Reads board components from the original owners selected by the Record filter.
pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<MissionBoardRead<'source>, CatalogError> {
    input.check_forge(forge)?;
    let mut read = MissionBoardRead::default();
    let Some(probe) = forge.pointer_probe(COMPONENTS, COMPONENT) else {
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
                read.failed(&admission, ExportSourceRole::SelectionCandidate, &error)?;
                continue;
            }
            Ok(true) => {}
        }
        let admission = RecordObservation::capture(input, &record)?;
        match trees.build(&record) {
            Ok(tree) => read.project(&tree, input, &admission, cancel)?,
            Err(error) => read.failed(&admission, DEPENDENCY, &error)?,
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

impl<'source> MissionBoardRead<'source> {
    fn project(
        &mut self,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
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
            |component| matches!(component, Value::Struct(value) if value.type_name() == COMPONENT),
        ) {
            originals.check_cancel()?;
            let component = originals.fact(component)?;
            let owner = ComponentOwner::read(tree, &context, &originals, component)?;
            self.rows.push(MissionBoard::read(
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
            ConfigurationInspection::MissionBoard,
            cancel,
        )?);
        source_projection_notes::record_dependency(admission, tree, notes, &mut self.ledger)?;
        self.ledger.observe(
            admission
                .source(None, DEPENDENCY, SourceDisposition::Projected)?
                .for_configuration(ConfigurationInspection::MissionBoard),
        );
        Ok(())
    }

    fn failed(
        &mut self,
        admission: &RecordObservation<'_, 'source>,
        role: ExportSourceRole,
        error: &DataForgeError,
    ) -> Result<(), CatalogError> {
        self.ledger.observe(
            admission
                .source(None, role, SourceDisposition::ReadFailed)?
                .diagnosed(export_source_failure::diagnosis(error)),
        );
        self.failures
            .push(CatalogReadFailure::of(admission.record(), error));
        Ok(())
    }
}
