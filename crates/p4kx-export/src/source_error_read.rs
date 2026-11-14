// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole, SourceUses, Value, ValueError};

use crate::catalog_read::original::{OriginalErrorParameter, OriginalValueFact};
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

use super::error::{OriginalErrorPool, OriginalErrorTable, OriginalValueErrorFacts};
use super::error_facts::ErrorFacts;

pub(crate) fn read(
    site: &OriginalValueFact<'_, '_, '_>,
    sources: &SourceUses,
) -> Result<OriginalValueErrorFacts, CatalogError> {
    let Value::Error(error) = site.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    convert(
        error,
        &ErrorFacts {
            site: Some(site),
            sources,
        },
    )
}

fn convert(
    error: &ValueError,
    facts: &ErrorFacts<'_, '_, '_, '_>,
) -> Result<OriginalValueErrorFacts, CatalogError> {
    Ok(match *error {
        ValueError::InvalidString { table, offset } => OriginalValueErrorFacts::InvalidString {
            table: OriginalErrorTable(table),
            offset: facts.parameter(
                OriginalErrorParameter::Offset,
                offset,
                SourceUseRole::Payload,
            )?,
        },
        ValueError::UnknownEnum { index } => OriginalValueErrorFacts::UnknownEnum {
            index: facts.declaration(index)?,
        },
        ValueError::UnknownStruct { index } => OriginalValueErrorFacts::UnknownStruct {
            index: facts.declaration(index)?,
        },
        ValueError::InvalidBool { byte } => OriginalValueErrorFacts::InvalidBool {
            byte: facts.parameter(OriginalErrorParameter::Byte, byte, SourceUseRole::Payload)?,
        },
        ValueError::InvalidPointer { .. }
        | ValueError::OwnedTwice { .. }
        | ValueError::OutsideRecord { .. } => return pointer(error, facts),
        ValueError::ArrayOutOfRange {
            first,
            count,
            len,
            pool,
        } => return array(first, count, len, pool, facts),
        ValueError::InlineClassCycle { struct_index } => {
            OriginalValueErrorFacts::InlineClassCycle {
                struct_index: facts.inline_class(struct_index)?,
            }
        }
        ValueError::DanglingReference { guid } => OriginalValueErrorFacts::DanglingReference {
            guid: facts.parameter(
                OriginalErrorParameter::Guid,
                CatalogGuid(guid),
                SourceUseRole::Payload,
            )?,
        },
        ValueError::PathTooWide { limit } => OriginalValueErrorFacts::PathTooWide {
            limit: limit_value(limit)?,
        },
        ValueError::UnexpectedPointerType { .. }
        | ValueError::UnexpectedReferenceType { .. }
        | ValueError::InstanceTooShort { .. } => return validation(error, facts),
        _ => return Err(ExportAccountingError::UnsupportedLootSourceValue.into()),
    })
}

fn pointer(
    error: &ValueError,
    facts: &ErrorFacts<'_, '_, '_, '_>,
) -> Result<OriginalValueErrorFacts, CatalogError> {
    let (ValueError::InvalidPointer {
        struct_index,
        instance_index,
    }
    | ValueError::OwnedTwice {
        struct_index,
        instance_index,
    }
    | ValueError::OutsideRecord {
        struct_index,
        instance_index,
    }) = *error
    else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let struct_index = facts.pointer_struct(struct_index)?;
    let instance_index = facts.pointer_instance(instance_index)?;
    Ok(match error {
        ValueError::InvalidPointer { .. } => OriginalValueErrorFacts::InvalidPointer {
            struct_index,
            instance_index,
        },
        ValueError::OwnedTwice { .. } => OriginalValueErrorFacts::OwnedTwice {
            struct_index,
            instance_index,
        },
        ValueError::OutsideRecord { .. } => OriginalValueErrorFacts::OutsideRecord {
            struct_index,
            instance_index,
        },
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    })
}

fn array(
    first: u32,
    count: u32,
    len: u64,
    pool: &'static str,
    facts: &ErrorFacts<'_, '_, '_, '_>,
) -> Result<OriginalValueErrorFacts, CatalogError> {
    Ok(OriginalValueErrorFacts::ArrayOutOfRange {
        first: facts.parameter(OriginalErrorParameter::First, first, SourceUseRole::Payload)?,
        count: facts.parameter(OriginalErrorParameter::Count, count, SourceUseRole::Payload)?,
        len: facts.parameter(
            OriginalErrorParameter::Len,
            DecimalU64::new(len),
            SourceUseRole::Conversion,
        )?,
        pool: OriginalErrorPool(pool),
    })
}

fn validation(
    error: &ValueError,
    facts: &ErrorFacts<'_, '_, '_, '_>,
) -> Result<OriginalValueErrorFacts, CatalogError> {
    Ok(match *error {
        ValueError::UnexpectedPointerType {
            struct_index,
            declared,
        } => OriginalValueErrorFacts::UnexpectedPointerType {
            struct_index: facts.unbound(struct_index),
            declared: facts.unbound(declared),
        },
        ValueError::UnexpectedReferenceType {
            guid,
            struct_index,
            declared,
        } => OriginalValueErrorFacts::UnexpectedReferenceType {
            guid: facts.unbound(CatalogGuid(guid)),
            struct_index: facts.unbound(struct_index),
            declared: facts.unbound(declared),
        },
        ValueError::InstanceTooShort {
            struct_index,
            size,
            needed,
        } => OriginalValueErrorFacts::InstanceTooShort {
            struct_index: facts.unbound(struct_index),
            size: facts.unbound(DecimalU64::new(size)),
            needed: facts.unbound(DecimalU64::new(needed)),
        },
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    })
}

fn limit_value(value: usize) -> Result<DecimalU64, CatalogError> {
    Ok(DecimalU64::new(
        u64::try_from(value).map_err(|_| ExportAccountingError::CounterOverflow)?,
    ))
}

#[cfg(test)]
#[path = "original_value_error_read_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_inline_cycle_tests.rs"]
mod inline_tests;
