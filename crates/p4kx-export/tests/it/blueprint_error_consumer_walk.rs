// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::OriginalValueErrorFacts;

use super::TestResult;
use crate::canonical_header::wire::Wire;

pub(super) fn visit_error(error: &OriginalValueErrorFacts, wire: Wire<'_>) -> TestResult {
    wire.raw(error)?;
    match error {
        OriginalValueErrorFacts::InvalidString { table, offset } => {
            wire.field("table")?.raw(table.as_str())?;
            wire.field("offset")?.leaf(offset)
        }
        OriginalValueErrorFacts::UnknownEnum { index }
        | OriginalValueErrorFacts::UnknownStruct { index } => wire.field("index")?.leaf(index),
        OriginalValueErrorFacts::InvalidBool { byte } => wire.field("byte")?.leaf(byte),
        OriginalValueErrorFacts::ArrayOutOfRange {
            first,
            count,
            len,
            pool,
        } => {
            wire.field("first")?.leaf(first)?;
            wire.field("count")?.leaf(count)?;
            wire.field("len")?
                .cell(len, |length, wire| wire.raw(&length.get().to_string()))?;
            wire.field("pool")?.raw(pool.as_str())
        }
        OriginalValueErrorFacts::InlineClassCycle { struct_index } => {
            wire.field("structIndex")?.leaf(struct_index)
        }
        OriginalValueErrorFacts::DanglingReference { guid } => wire
            .field("guid")?
            .cell(guid, |guid, wire| wire.raw(&guid.get().to_string())),
        OriginalValueErrorFacts::PathTooWide { limit } => {
            wire.field("limit")?.raw(&limit.get().to_string())
        }
        OriginalValueErrorFacts::InstanceTooShort {
            struct_index,
            size,
            needed,
        } => {
            wire.field("structIndex")?.leaf(struct_index)?;
            wire.field("size")?
                .cell(size, |size, wire| wire.raw(&size.get().to_string()))?;
            wire.field("needed")?
                .cell(needed, |needed, wire| wire.raw(&needed.get().to_string()))
        }
        other => visit_pointer_error(other, wire),
    }
}

fn visit_pointer_error(error: &OriginalValueErrorFacts, wire: Wire<'_>) -> TestResult {
    match error {
        OriginalValueErrorFacts::InvalidPointer {
            struct_index,
            instance_index,
        }
        | OriginalValueErrorFacts::OwnedTwice {
            struct_index,
            instance_index,
        }
        | OriginalValueErrorFacts::OutsideRecord {
            struct_index,
            instance_index,
        } => {
            wire.field("structIndex")?.leaf(struct_index)?;
            wire.field("instanceIndex")?.leaf(instance_index)
        }
        OriginalValueErrorFacts::UnexpectedPointerType {
            struct_index,
            declared,
        } => {
            wire.field("structIndex")?.leaf(struct_index)?;
            wire.field("declared")?.leaf(declared)
        }
        OriginalValueErrorFacts::UnexpectedReferenceType {
            guid,
            struct_index,
            declared,
        } => {
            wire.field("guid")?
                .cell(guid, |guid, wire| wire.raw(&guid.get().to_string()))?;
            wire.field("structIndex")?.leaf(struct_index)?;
            wire.field("declared")?.leaf(declared)
        }
        _ => Err("new original error facts require a typed traversal".into()),
    }
}
