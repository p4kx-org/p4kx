// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, MAX_NESTING_DEPTH, MAX_RECORD_VALUES, Record, RecordTree, Reference, StructValue,
    Value,
};

use crate::CatalogError;
use crate::catalog_number_capture::FloatToken;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity, ExportSourceRole};
use crate::export_field_collect::FieldCollector;
use crate::export_ledger::{ExportAccountingError, ExportLedger};
use crate::number_position::NumberPosition;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;

/// Inspects only materialized owned values, retaining field failures independently of row mapping.
pub(crate) fn inspect(
    tree: &RecordTree<'_>,
    record: &Record<'_>,
    kind: Option<Kind>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<ExportLedger, CatalogError> {
    if (tree.record_index(), tree.guid()) != (record.index(), record.guid()) {
        return Err(ExportAccountingError::ConflictingFieldInspection.into());
    }
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut walker = Walker {
        collector: FieldCollector::new(tree, record.index(), kind, role),
        visited: 0,
        cancel,
    };
    walker.structure(
        tree.root(),
        &mut String::new(),
        0,
        None,
        NumberPosition::Root,
    )?;
    let mut ledger = ExportLedger::default();
    ledger.field_inspections.push(walker.collector.finish());
    Ok(ledger)
}

struct Walker<'t, 'f, 'c> {
    collector: FieldCollector<'t, 'f>,
    visited: usize,
    cancel: Option<&'c CancelToken>,
}

impl Walker<'_, '_, '_> {
    fn structure(
        &mut self,
        structure: &StructValue<'_>,
        path: &mut String,
        depth: usize,
        parent_type: Option<&str>,
        position: NumberPosition,
    ) -> Result<bool, CatalogError> {
        if depth > MAX_NESTING_DEPTH {
            self.collector.limited(path, structure.type_name());
            return Ok(false);
        }
        for field in structure.fields() {
            let length = path.len();
            if !path.is_empty() {
                path.push('.');
            }
            path.push_str(field.name());
            let position = position.field(structure.type_name(), field.name());
            let complete = self.value(
                field.value(),
                path,
                structure.type_name(),
                depth,
                parent_type,
                position,
            )?;
            path.truncate(length);
            if !complete {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn value(
        &mut self,
        value: &Value<'_>,
        path: &mut String,
        source_type: &str,
        depth: usize,
        parent_type: Option<&str>,
        position: NumberPosition,
    ) -> Result<bool, CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if self.visited == MAX_RECORD_VALUES {
            self.collector.limited(path, source_type);
            return Ok(false);
        }
        self.visited += 1;
        match value {
            Value::Struct(structure) => {
                self.structure(structure, path, depth + 1, Some(source_type), position)
            }
            Value::Array(values) => {
                self.array(values, path, source_type, depth, parent_type, position)
            }
            _ => Ok(self.scalar(
                value,
                path,
                source_type,
                parent_type,
                position.contract(value),
            )),
        }
    }

    fn scalar(
        &mut self,
        value: &Value<'_>,
        path: &str,
        source_type: &str,
        parent_type: Option<&str>,
        contract: Option<ResourceNumberContract>,
    ) -> bool {
        if let Some(token) = FloatToken::read(value) {
            self.collector
                .non_finite(path, source_type, contract, token);
            return true;
        }
        match value {
            Value::Null => self.collector.note(
                path,
                source_type,
                ExportDiagnosticCode::OptionalAbsent,
                ExportDiagnosticSeverity::Information,
                None,
            ),
            Value::Guid(guid) if *guid == p4kx_extraction::Guid::default() => self.collector.note(
                path,
                source_type,
                ExportDiagnosticCode::EmptyValue,
                ExportDiagnosticSeverity::Information,
                None,
            ),
            Value::String(text) | Value::Locale(text) | Value::Enum(text) if text.is_empty() => {
                self.collector.note(
                    path,
                    source_type,
                    ExportDiagnosticCode::EmptyValue,
                    ExportDiagnosticSeverity::Information,
                    None,
                );
            }
            Value::Error(error) => {
                crate::export_field_error::record(&mut self.collector, path, source_type, *error);
            }
            Value::Reference(reference) => {
                return self.reference(reference, path, source_type, parent_type);
            }
            _ => {}
        }
        true
    }

    fn reference(
        &mut self,
        reference: &Reference<'_>,
        path: &str,
        source_type: &str,
        parent_type: Option<&str>,
    ) -> bool {
        if reference.record_index() != self.collector.owner_index()
            && reference.location().is_some()
        {
            self.collector.note(
                path,
                source_type,
                ExportDiagnosticCode::OutsideOwner,
                ExportDiagnosticSeverity::Information,
                Some(reference.guid()),
            );
            self.collector.foreign(path, source_type, reference);
            true
        } else {
            self.collector
                .catalog_reference(path, source_type, parent_type, reference)
        }
    }

    fn array(
        &mut self,
        values: &[Value<'_>],
        path: &mut String,
        source_type: &str,
        depth: usize,
        parent_type: Option<&str>,
        position: NumberPosition,
    ) -> Result<bool, CatalogError> {
        use std::fmt::Write as _;

        if values.is_empty() {
            self.collector.note(
                path,
                source_type,
                ExportDiagnosticCode::EmptyValue,
                ExportDiagnosticSeverity::Information,
                None,
            );
        }
        let indexed = values.iter().enumerate();
        for (index, value) in indexed {
            let length = path.len();
            write!(path, "[{index}]").map_err(|error| {
                CatalogError::Accounting(ExportAccountingError::FieldPath(error))
            })?;
            let complete = self.value(
                value,
                path,
                source_type,
                depth,
                parent_type,
                position.element(),
            )?;
            path.truncate(length);
            if !complete {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
