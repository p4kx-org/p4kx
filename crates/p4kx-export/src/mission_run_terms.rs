// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::record_fields::{integer_value, number_value};
use crate::{CatalogError, ExportAccountingError};

/// The run terms selected from the original contract for its row and its sub contracts.
#[derive(Debug)]
pub(crate) struct RunTerms {
    pub(crate) time_to_complete: SourcedValue<f32>,
    pub(crate) buy_in_amount: SourcedValue<i64>,
}

impl RunTerms {
    pub(crate) fn read<'source>(
        fields: &OriginalFlowFields<'_, '_, 'source>,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        let owner = fields
            .owner_of(contract)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut uses = Vec::new();
        let block = if let Some((_, results)) = fields.field(Some(owner), "contractResults")? {
            consult(&mut uses, results)?;
            fields
                .first_struct_observed(results, |value| consult(&mut uses, fields.fact(value)?))?
        } else {
            retain(&mut uses, fields.survey(Some(owner))?)?;
            None
        };
        let prefix = uses.len();
        let time_to_complete = term(fields, block, "timeToComplete", &mut uses, time)?;
        uses.truncate(prefix);
        let buy_in_amount = term(fields, block, "contractBuyInAmount", &mut uses, |value| {
            integer_value(value).map(|amount| (amount, SourceUseRole::Payload))
        })?;
        Ok(Self {
            time_to_complete,
            buy_in_amount,
        })
    }
}

fn time(value: &Value<'_>) -> Option<(f32, SourceUseRole)> {
    let role = if matches!(value, Value::Single(_)) {
        SourceUseRole::Payload
    } else {
        SourceUseRole::Conversion
    };
    Some((number_value(value)?, role))
}

fn term<'scope, 'read, 'source, T>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    block: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    name: &str,
    uses: &mut Vec<SourceUse>,
    decode: impl FnOnce(&Value<'source>) -> Option<(T, SourceUseRole)>,
) -> Result<SourcedValue<T>, CatalogError> {
    if let Some(block) = block {
        if let Some((_, fact)) = fields.field(Some(block), name)? {
            if let Some((value, role)) = decode(fact.value()) {
                retain(uses, fact.observe(OriginalValuePart::Value, role)?)?;
                return Ok(fields
                    .sources()
                    .value(SourceValue::Present(value), uses, true));
            }
            consult(uses, fact)?;
        } else {
            retain(uses, fields.survey(Some(block))?)?;
        }
    }
    let complete = !uses
        .iter()
        .any(|used| used.role() == SourceUseRole::Attempted);
    let value = if complete {
        SourceValue::Absent
    } else {
        SourceValue::Unreadable
    };
    Ok(fields.sources().value(value, uses, complete))
}

fn consult(
    uses: &mut Vec<SourceUse>,
    fact: &OriginalValueFact<'_, '_, '_>,
) -> Result<(), CatalogError> {
    let role = if matches!(fact.value(), Value::Error(_)) {
        SourceUseRole::Attempted
    } else {
        SourceUseRole::Selector
    };
    retain(uses, fact.observe(OriginalValuePart::Value, role)?)
}

fn retain(uses: &mut Vec<SourceUse>, source: SourceUse) -> Result<(), CatalogError> {
    uses.try_reserve(1)
        .map_err(CatalogError::SourceUseAllocation)?;
    uses.push(source);
    Ok(())
}
