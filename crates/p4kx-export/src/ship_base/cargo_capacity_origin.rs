// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, FieldOrigin, Record, RecordTree, SourceUse, SourceUseRole, SourceValue,
    SourcedValue, StructValue, Value,
};

use super::cargo_container::{ContainerInput, MeasuredCapacity, OperandRole};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn read<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    cancel: Option<&CancelToken>,
) -> Result<(MeasuredCapacity<'source>, CapacitySource), CatalogError> {
    let context = input.record_context(record)?;
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let mut evidence =
        vec![context.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?];
    let mut complete = true;
    let mut error = None;
    let capacity = ContainerInput::select(tree.root(), &mut |parent, name, value, operand| {
        if error.is_some() {
            return;
        }
        if matches!(value, Some(Value::Error(_))) {
            complete = false;
        }
        match observe(&fields, parent, name, value, operand, &mut evidence) {
            Ok(()) => {}
            Err(failure) => error = Some(failure),
        }
    })
    .measure();
    if let Some(error) = error {
        return Err(error);
    }
    let source = CapacitySource {
        record_index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(record.index()),
        )?,
        origin: fields.sources().origin(&evidence, complete),
    };
    Ok((capacity, source))
}

#[derive(Debug)]
pub(super) struct CapacitySource {
    pub(super) record_index: SourcedValue<u32>,
    pub(super) origin: FieldOrigin,
}

fn observe(
    fields: &OriginalFlowFields<'_, '_, '_>,
    parent: &StructValue<'_>,
    name: &'static str,
    value: Option<&Value<'_>>,
    operand: OperandRole,
    evidence: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    let owner = if std::ptr::addr_eq(parent, fields.root()) {
        None
    } else {
        Some(
            fields
                .facts()
                .iter()
                .find(|fact| {
                    matches!(fact.value(), Value::Struct(source)
                if std::ptr::addr_eq(source, parent))
                })
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
        )
    };
    match (value, fields.field(owner, name)?) {
        (Some(value), Some((_, fact))) if std::ptr::addr_eq(value, fact.value()) => {
            let (part, role) = match (operand, value) {
                (_, Value::Error(_)) => (OriginalValuePart::Value, SourceUseRole::Attempted),
                (OperandRole::Kind, Value::Struct(_)) => {
                    (OriginalValuePart::ConcreteType, SourceUseRole::Selector)
                }
                (OperandRole::Amount, _) => (OriginalValuePart::Value, SourceUseRole::Conversion),
                _ => (OriginalValuePart::Value, SourceUseRole::Selector),
            };
            evidence.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
            evidence.push(fact.observe(part, role)?);
        }
        (None, None) => evidence.push(fields.survey(owner)?),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
    Ok(())
}
