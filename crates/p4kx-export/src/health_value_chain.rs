// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use super::OriginalValueFact;
use crate::ExportAccountingError;

pub(super) fn check<'scope, 'read, 'source>(
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
) -> Result<(), ExportAccountingError> {
    let mut previous = None;
    for ancestor in ancestors {
        if !std::ptr::eq(ancestor.scope(), fact.scope())
            || ancestor.parent_ordinal() != previous
            || ancestor.ordinal() >= fact.ordinal()
            || previous.is_some_and(|previous| previous >= ancestor.ordinal())
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        previous = Some(ancestor.ordinal());
    }
    if fact.parent_ordinal() != previous {
        return Err(ExportAccountingError::ConflictingSourceDatum);
    }
    Ok(())
}

pub(super) fn parent<'fact, 'scope, 'read, 'source>(
    source: &StructValue<'source>,
    fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
    ancestors: &[&'fact OriginalValueFact<'scope, 'read, 'source>],
) -> Result<Option<&'fact OriginalValueFact<'scope, 'read, 'source>>, ExportAccountingError> {
    check(fact, ancestors)?;
    let parent = ancestors
        .iter()
        .rev()
        .copied()
        .find(|ancestor| matches!(ancestor.value(), Value::Struct(_)));
    let expected = match parent.map(OriginalValueFact::value) {
        Some(Value::Struct(value)) => value,
        None => fact.scope().tree().root(),
        _ => return Err(ExportAccountingError::ConflictingSourceDatum),
    };
    if !std::ptr::eq(expected, source)
        || !source
            .fields()
            .iter()
            .any(|field| std::ptr::eq(field, fact.field()))
    {
        return Err(ExportAccountingError::ConflictingSourceDatum);
    }
    Ok(parent)
}
