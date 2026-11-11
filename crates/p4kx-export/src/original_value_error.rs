// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// The exact negative parser-table identifier from an original error.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct OriginalErrorTable(pub(crate) &'static str);

/// The exact negative parser-pool identifier from an original error.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct OriginalErrorPool(pub(crate) &'static str);

/// Complete typed original error parameters at one source occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum OriginalValueErrorFacts {
    InvalidString {
        table: OriginalErrorTable,
        offset: SourcedValue<u32>,
    },
    UnknownEnum {
        index: SourcedValue<u16>,
    },
    UnknownStruct {
        index: SourcedValue<u16>,
    },
    InvalidBool {
        byte: SourcedValue<u8>,
    },
    InvalidPointer {
        struct_index: SourcedValue<u32>,
        instance_index: SourcedValue<u32>,
    },
    ArrayOutOfRange {
        first: SourcedValue<u32>,
        count: SourcedValue<u32>,
        len: SourcedValue<DecimalU64>,
        pool: OriginalErrorPool,
    },
    OwnedTwice {
        struct_index: SourcedValue<u32>,
        instance_index: SourcedValue<u32>,
    },
    InlineClassCycle {
        struct_index: SourcedValue<u32>,
    },
    DanglingReference {
        guid: SourcedValue<CatalogGuid>,
    },
    OutsideRecord {
        struct_index: SourcedValue<u32>,
        instance_index: SourcedValue<u32>,
    },
    UnexpectedPointerType {
        struct_index: SourcedValue<u32>,
        declared: SourcedValue<u32>,
    },
    UnexpectedReferenceType {
        guid: SourcedValue<CatalogGuid>,
        struct_index: SourcedValue<u32>,
        declared: SourcedValue<u32>,
    },
    PathTooWide {
        limit: DecimalU64,
    },
    InstanceTooShort {
        struct_index: SourcedValue<u32>,
        size: SourcedValue<DecimalU64>,
        needed: SourcedValue<DecimalU64>,
    },
}
