// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DecimalU64, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::record_fields::{integer_value, number_value};
use crate::source_value_catalog::SourceValueRef;

pub(crate) fn weight<'source>(
    source: &OriginalFlowFields<'_, '_, 'source>,
    entry: &StructValue<'source>,
) -> Result<(Option<f32>, SourcedValue<SourceValueRef>), CatalogError> {
    let mut uses = Vec::new();
    let owner = source.owner_of(entry)?;
    let (number, value) = match declaration(source, owner, "weight", &mut uses)? {
        Some(fact) => {
            let number = number_value(fact.value());
            let (value, role) = match (number, fact.value()) {
                (Some(_), Value::Single(_)) => (reference(fact), SourceUseRole::Payload),
                (Some(_), _) => (reference(fact), SourceUseRole::Conversion),
                (None, Value::Null) => (SourceValue::Null, SourceUseRole::Selector),
                (None, Value::Error(_)) => (SourceValue::Unreadable, SourceUseRole::Attempted),
                (None, _) => (SourceValue::Invalid, SourceUseRole::Selector),
            };
            uses.push(fact.observe(OriginalValuePart::Value, role)?);
            (number, value)
        }
        None => (None, SourceValue::Absent),
    };
    Ok((number, sourced(source, value, &uses)))
}

pub(crate) fn dupe_limit<'source>(
    source: &OriginalFlowFields<'_, '_, 'source>,
    entry: &StructValue<'source>,
) -> Result<(Option<i64>, SourcedValue<SourceValueRef>), CatalogError> {
    let mut uses = Vec::new();
    let owner = source.owner_of(entry)?;
    let (number, value) = match declaration(source, owner, "optionalData", &mut uses)? {
        Some(fact) => {
            uses.push(selector(fact)?);
            match fact.value() {
                Value::Array(slots) => first_dupe_limit(source, slots, &mut uses)?,
                Value::Null => (None, SourceValue::Null),
                Value::Error(_) => (None, SourceValue::Unreadable),
                _ => (None, SourceValue::Invalid),
            }
        }
        None => (None, SourceValue::Absent),
    };
    Ok((number, sourced(source, value, &uses)))
}

fn first_dupe_limit<'source>(
    source: &OriginalFlowFields<'_, '_, 'source>,
    slots: &[Value<'source>],
    uses: &mut Vec<SourceUse>,
) -> Result<(Option<i64>, SourceValue<SourceValueRef>), CatalogError> {
    let mut unselected = SourceValue::Absent;
    for slot in slots {
        let slot_fact = source.fact(slot)?;
        uses.push(selector(slot_fact)?);
        if !matches!(slot, Value::Struct(_)) {
            continue;
        }
        let Some(candidate) = declaration(source, Some(slot_fact), "dupeLimit", uses)? else {
            continue;
        };
        if let Some(number) = integer_value(candidate.value()) {
            let role = if matches!(candidate.value(), Value::Int64(_)) {
                SourceUseRole::Payload
            } else {
                SourceUseRole::Conversion
            };
            uses.push(candidate.observe(OriginalValuePart::Value, role)?);
            return Ok((Some(number), reference(candidate)));
        }
        uses.push(selector(candidate)?);
        unselected = match candidate.value() {
            Value::Null if matches!(unselected, SourceValue::Absent) => SourceValue::Null,
            Value::Null | Value::Error(_) => unselected,
            _ => SourceValue::Invalid,
        };
    }
    let state = if incomplete(uses) {
        SourceValue::Unreadable
    } else {
        unselected
    };
    Ok((None, state))
}

fn declaration<'fields, 'scope, 'read, 'source>(
    source: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    name: &str,
    uses: &mut Vec<SourceUse>,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let Some((_, fact)) = source.field(owner, name)? else {
        uses.push(source.survey(owner)?);
        return Ok(None);
    };
    uses.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
    Ok(Some(fact))
}

fn selector(fact: &OriginalValueFact<'_, '_, '_>) -> Result<SourceUse, CatalogError> {
    let role = if matches!(fact.value(), Value::Error(_)) {
        SourceUseRole::Attempted
    } else {
        SourceUseRole::Selector
    };
    fact.observe(OriginalValuePart::Value, role)
}

const fn reference(fact: &OriginalValueFact<'_, '_, '_>) -> SourceValue<SourceValueRef> {
    SourceValue::Present(SourceValueRef {
        value_index: DecimalU64::new(fact.ordinal()),
    })
}

fn sourced(
    source: &OriginalFlowFields<'_, '_, '_>,
    value: SourceValue<SourceValueRef>,
    uses: &[SourceUse],
) -> SourcedValue<SourceValueRef> {
    source.sources().value(value, uses, !incomplete(uses))
}

fn incomplete(uses: &[SourceUse]) -> bool {
    uses.iter()
        .any(|input| input.role() == SourceUseRole::Attempted)
}
