// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeSet;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Record, RecordTree, TreeBuilder,
};

use super::authored_fields::Observed;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
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
use crate::{CatalogError, CatalogReadFailure};

const ROLE: ExportSourceRole = ExportSourceRole::Dependency;

pub(super) struct Definitions<'source, Row> {
    pub(super) rows: Vec<(&'source str, Row)>,
    pub(super) numbers: Vec<CapturedNumber>,
    pub(super) failures: Vec<CatalogReadFailure<'source>>,
    pub(super) ledger: ExportLedger,
}

pub(super) type Projection<'source, Row> = fn(
    &'source str,
    CanonicalRecordHeader<'source>,
    Observed<'_, '_, '_, 'source>,
    &mut Vec<FieldError>,
) -> Result<Row, CatalogError>;

pub(super) struct Selection<'selection> {
    pub(super) source_types: &'selection [&'static str],
    pub(super) linked: &'selection BTreeSet<u32>,
    pub(super) filter: Option<&'selection Filter>,
    pub(super) inspection: ConfigurationInspection,
}

impl Selection<'_> {
    fn admits(&self, record: &Record<'_>) -> Result<bool, DataForgeError> {
        if let Some(filter) = self.filter
            && !self.linked.contains(&record.index())
        {
            return Ok(filter.matches(record.record_path()?));
        }
        Ok(true)
    }
}

/// Reads admitted physical definition Records in source order.
pub(super) fn read<'source, Row>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    selection: &Selection<'_>,
    cancel: Option<&CancelToken>,
    project: Projection<'source, Row>,
) -> Result<Definitions<'source, Row>, CatalogError> {
    input.check_forge(forge)?;
    let mut result = Definitions {
        rows: Vec::new(),
        numbers: Vec::new(),
        failures: Vec::new(),
        ledger: ExportLedger::default(),
    };
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let Ok(source_type) = record.type_name() else {
            continue;
        };
        if !selection.source_types.contains(&source_type) {
            continue;
        }
        let admitted = selection.admits(&record);
        if matches!(admitted, Ok(false)) {
            continue;
        }
        let admission = RecordObservation::capture(input, &record)?;
        if let Err(error) = admitted {
            result.failed(
                &admission,
                selection.inspection,
                ExportSourceRole::SelectionCandidate,
                &error,
            )?;
            continue;
        }
        match trees.build(&record) {
            Ok(tree) => {
                let emitted = read_row(
                    &tree,
                    input,
                    &admission,
                    selection.inspection,
                    cancel,
                    project,
                )?;
                result.rows.push((source_type, emitted.row));
                result.numbers.extend(emitted.numbers);
                result.ledger.merge(emitted.ledger);
            }
            Err(error) => result.failed(&admission, selection.inspection, ROLE, &error)?,
        }
    }
    Ok(result)
}

impl<'source, Row> Definitions<'source, Row> {
    fn failed(
        &mut self,
        admission: &RecordObservation<'_, 'source>,
        inspection: ConfigurationInspection,
        role: ExportSourceRole,
        error: &DataForgeError,
    ) -> Result<(), CatalogError> {
        self.ledger.observe(
            admission
                .source(None, role, SourceDisposition::ReadFailed)?
                .for_configuration(inspection)
                .diagnosed(export_source_failure::diagnosis(error)),
        );
        self.failures
            .push(CatalogReadFailure::of(admission.record(), error));
        Ok(())
    }
}

struct RowRead<Row> {
    row: Row,
    numbers: Vec<CapturedNumber>,
    ledger: ExportLedger,
}

fn read_row<'source, Row>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    inspection: ConfigurationInspection,
    cancel: Option<&CancelToken>,
    project: Projection<'source, Row>,
) -> Result<RowRead<Row>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::configuration(&originals, input, &capture, &numbers);
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let mut notes = Vec::new();
    let row = project(tree.type_name(), source, fields, &mut notes)?;
    let values = 0..originals.facts().len();
    let spans = (!values.is_empty()).then_some(values);
    let mut ledger = export_field_walk::inspect_configuration(
        input,
        tree,
        admission,
        spans.as_slice(),
        inspection,
        cancel,
    )?;
    source_projection_notes::record_dependency(admission, tree, notes, &mut ledger)?;
    ledger.observe(
        admission
            .source(None, ROLE, SourceDisposition::Projected)?
            .for_configuration(inspection),
    );
    Ok(RowRead {
        row,
        numbers: numbers.into_inner(),
        ledger,
    })
}
