// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{CancelToken, Guid, Record, RecordTree, Reference, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::FloatToken;
use crate::catalog_read::original::{
    ExampleBodyReceipt, OriginalRecordScope, OriginalValueEdge, OriginalValueEvent,
    OriginalValueSite, OwnedValueEvent, OwnedValueVisit, OwnedValueWalk, ReferenceCapture,
};
use crate::catalog_reference;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity, ExportSourceRole};
use crate::export_field_collect::FieldCollector;
use crate::export_ledger::{ExportAccountingError, ExportLedger};
use crate::number_position::NumberPosition;
use crate::record_observation::RecordObservation;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;

#[path = "export_field_selection.rs"]
mod selection;

use selection::FieldSelection;
pub(crate) use selection::inspect_record_field;

/// Inspects only materialized owned values, retaining field failures independently of row mapping.
#[cfg(test)]
pub(crate) fn inspect(
    tree: &RecordTree<'_>,
    record: &Record<'_>,
    kind: Option<Kind>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    if !tree.is_from(record) {
        return Err(ExportAccountingError::ConflictingFieldInspection.into());
    }
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let admission = RecordObservation::accounting(record);
    inspect_observed(None, tree, &admission, kind, role, cancel)
}

/// Inspects a canonical body paired with its actual original header admission.
pub(crate) fn inspect_observed<'source>(
    input: Option<&OriginalCatalogInput<'_, '_>>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    kind: Option<Kind>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    inspect_values(
        input,
        tree,
        admission,
        kind,
        role,
        cancel,
        &FieldSelection::All,
    )
}

/// Inspects original spans of one owner's independent configurations in source order.
pub(crate) fn inspect_configuration<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    spans: &[Range<usize>],
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    if spans.iter().any(Range::is_empty)
        || spans
            .windows(2)
            .any(|pair| matches!(pair, [first, next] if first.end > next.start))
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    inspect_values(
        Some(input),
        tree,
        admission,
        None,
        ExportSourceRole::Dependency,
        cancel,
        &FieldSelection::Subtrees(spans),
    )
}

fn inspect_values<'source>(
    input: Option<&OriginalCatalogInput<'_, '_>>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    kind: Option<Kind>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
    selection: &FieldSelection<'_, '_>,
) -> Result<ExportLedger, CatalogError> {
    admission.check_tree(tree)?;
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut inspector = Inspector {
        collector: FieldCollector::new(tree, admission, kind, role)?,
        positions: NumberPositions::default(),
    };
    if let Some(input) = input {
        let scope = input.record_scope(admission.record(), tree)?;
        let mut capture = ReferenceCapture::new(input, admission, &scope)?;
        inspector.observed(&scope, &mut capture, cancel, selection)?;
    } else {
        inspector.accounting(tree, admission.record(), cancel)?;
    }
    let mut ledger = ExportLedger::default();
    let mut inspection = inspector.collector.finish();
    inspection.root_field = selection.root_field();
    ledger.field_inspections.push(inspection);
    Ok(ledger)
}

struct Inspector<'tree, 'source> {
    collector: FieldCollector<'tree, 'source>,
    positions: NumberPositions,
}

impl Inspector<'_, '_> {
    fn observed(
        &mut self,
        scope: &OriginalRecordScope<'_, '_>,
        capture: &mut ReferenceCapture<'_, '_, '_>,
        cancel: Option<&CancelToken>,
        selection: &FieldSelection<'_, '_>,
    ) -> Result<(), CatalogError> {
        if selection.is_empty() {
            return Ok(());
        }
        let mut walk = scope.walk(cancel);
        while let Some(event) = walk.next_event()? {
            match event {
                OriginalValueEvent::Value(site) => {
                    self.visit(
                        site.neutral(),
                        Some((capture, &site)),
                        selection.includes(&site)?,
                    )?;
                    if matches!(site.value(), Value::Struct(_) | Value::Array(_)) {
                        capture.enter(&site)?;
                    }
                }
                OriginalValueEvent::EndContainer { ordinal, .. } => {
                    self.positions.leave(ordinal)?;
                    capture.leave(ordinal)?;
                }
            }
        }
        Ok(())
    }

    fn accounting<'source>(
        &mut self,
        tree: &RecordTree<'source>,
        record: &Record<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let mut walk = OwnedValueWalk::new(record, tree, cancel)?;
        while let Some(event) = walk.next_event()? {
            match event {
                OwnedValueEvent::Value(visit) => self.visit(&visit, None, true)?,
                OwnedValueEvent::EndContainer { ordinal, .. } => self.positions.leave(ordinal)?,
            }
        }
        Ok(())
    }

    fn visit(
        &mut self,
        visit: &OwnedValueVisit<'_, '_, '_>,
        original: Option<(
            &ReferenceCapture<'_, '_, '_>,
            &OriginalValueSite<'_, '_, '_, '_>,
        )>,
        selected: bool,
    ) -> Result<(), CatalogError> {
        let position = self.positions.at(visit)?;
        match visit.value() {
            Value::Struct(_) => self.positions.enter(visit.ordinal(), position),
            Value::Array(values) => {
                if selected && values.is_empty() {
                    self.collector.note(
                        visit.logical_path(),
                        visit.owner_type(),
                        ExportDiagnosticCode::EmptyValue,
                        ExportDiagnosticSeverity::Information,
                        None,
                        example(original)?,
                    )?;
                }
                self.positions.enter(visit.ordinal(), position);
            }
            _ if selected => self.scalar(visit, position.contract(visit.value()), original)?,
            _ => {}
        }
        Ok(())
    }

    fn scalar(
        &mut self,
        visit: &OwnedValueVisit<'_, '_, '_>,
        contract: Option<ResourceNumberContract>,
        original: Option<(
            &ReferenceCapture<'_, '_, '_>,
            &OriginalValueSite<'_, '_, '_, '_>,
        )>,
    ) -> Result<(), CatalogError> {
        let value = visit.value();
        let path = visit.logical_path();
        let source_type = visit.owner_type();
        if let Some(token) = FloatToken::read(value) {
            self.collector.non_finite(
                path,
                source_type,
                visit.ordinal(),
                contract,
                token,
                example(original)?,
            );
            return Ok(());
        }
        if let Some(code) = informational(value) {
            self.collector.note(
                path,
                source_type,
                code,
                ExportDiagnosticSeverity::Information,
                None,
                example(original)?,
            )?;
        } else {
            match value {
                Value::Error(error) => {
                    let observation = match original {
                        Some((capture, site)) => capture.error(site)?,
                        None => self.collector.accounting_error(path, source_type, *error),
                    };
                    let body = match (original, observation.as_ref()) {
                        (Some(_), Some(reference)) => {
                            Some(ExampleBodyReceipt::reference(reference)?)
                        }
                        _ => example(original)?,
                    };
                    crate::export_field_error::record(
                        &mut self.collector,
                        path,
                        source_type,
                        *error,
                        body,
                    )?;
                    if let Some(observation) = observation {
                        self.collector.unresolved(observation);
                    }
                }
                Value::Reference(reference) => self.reference(reference, visit, original)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn reference(
        &mut self,
        reference: &Reference<'_>,
        visit: &OwnedValueVisit<'_, '_, '_>,
        original: Option<(
            &ReferenceCapture<'_, '_, '_>,
            &OriginalValueSite<'_, '_, '_, '_>,
        )>,
    ) -> Result<(), CatalogError> {
        let path = visit.logical_path();
        let source_type = visit.owner_type();
        let foreign = reference.record_index() != self.collector.owner_index()
            && reference.location().is_some();
        let kind =
            catalog_reference::target_kind(source_type, visit.parent_type(), path, reference);
        if !foreign && kind.is_none() {
            return Ok(());
        }
        let observation = match original {
            Some((capture, site)) => capture.reference(site)?,
            None => self
                .collector
                .accounting_reference(path, source_type, reference),
        };
        if foreign {
            self.collector.note(
                path,
                source_type,
                ExportDiagnosticCode::OutsideOwner,
                ExportDiagnosticSeverity::Information,
                Some(reference.guid()),
                match original {
                    Some(_) => Some(ExampleBodyReceipt::reference(&observation)?),
                    None => None,
                },
            )?;
            self.collector
                .unresolved(observation.resolved(crate::ExportReferenceStatus::OutsideOwner));
        } else if let Some(kind) = kind {
            self.collector.catalog_reference(kind, observation);
        }
        Ok(())
    }
}

fn example(
    original: Option<(
        &ReferenceCapture<'_, '_, '_>,
        &OriginalValueSite<'_, '_, '_, '_>,
    )>,
) -> Result<Option<ExampleBodyReceipt>, CatalogError> {
    original
        .map(|(capture, site)| capture.example(site))
        .transpose()
}

fn informational(value: &Value<'_>) -> Option<ExportDiagnosticCode> {
    match value {
        Value::Null => Some(ExportDiagnosticCode::OptionalAbsent),
        Value::Guid(guid) if *guid == Guid::default() => Some(ExportDiagnosticCode::EmptyValue),
        Value::String(text) | Value::Locale(text) | Value::Enum(text) if text.is_empty() => {
            Some(ExportDiagnosticCode::EmptyValue)
        }
        _ => None,
    }
}
struct ContainerPosition {
    ordinal: u64,
    position: NumberPosition,
}

/// Numeric contracts accumulated along the actual owned-value event path.
#[derive(Default)]
struct NumberPositions {
    containers: Vec<ContainerPosition>,
}

impl NumberPositions {
    fn at(
        &self,
        visit: &OwnedValueVisit<'_, '_, '_>,
    ) -> Result<NumberPosition, ExportAccountingError> {
        let parent = self.containers.last();
        if visit.parent_ordinal() != parent.map(|parent| parent.ordinal) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        let position = parent.map_or(NumberPosition::Root, |parent| parent.position);
        Ok(match visit.edge() {
            OriginalValueEdge::Field { .. } => {
                position.field(visit.owner_type(), visit.field().name())
            }
            OriginalValueEdge::Element { .. } => position.element(),
        })
    }

    fn enter(&mut self, ordinal: u64, position: NumberPosition) {
        self.containers
            .push(ContainerPosition { ordinal, position });
    }

    fn leave(&mut self, ordinal: u64) -> Result<(), ExportAccountingError> {
        let container = self
            .containers
            .pop()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if container.ordinal != ordinal {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "export_field_reference_inventory_tests.rs"]
mod reference_inventory_tests;
