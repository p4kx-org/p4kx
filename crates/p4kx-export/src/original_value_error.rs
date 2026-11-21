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
pub struct OriginalErrorTable(pub(crate) &'static str);

impl OriginalErrorTable {
    /// Borrows the original parser-table identifier.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

/// The exact negative parser-pool identifier from an original error.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct OriginalErrorPool(pub(crate) &'static str);

impl OriginalErrorPool {
    /// Borrows the original parser-pool identifier.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Complete typed original error parameters at one source occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum OriginalValueErrorFacts {
    /// An original string offset outside its parser table.
    InvalidString {
        /// The parser table named by the original error.
        table: OriginalErrorTable,
        /// The original string offset.
        offset: SourcedValue<u32>,
    },
    /// An original enum index without a matching declaration.
    UnknownEnum {
        /// The original enum index.
        index: SourcedValue<u16>,
    },
    /// An original structure index without a matching declaration.
    UnknownStruct {
        /// The original structure index.
        index: SourcedValue<u16>,
    },
    /// An original Boolean byte outside the supported representation.
    InvalidBool {
        /// The original byte.
        byte: SourcedValue<u8>,
    },
    /// An original pointer without a valid structure instance.
    InvalidPointer {
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The original target instance index.
        instance_index: SourcedValue<u32>,
    },
    /// An original array range outside its parser pool.
    ArrayOutOfRange {
        /// The original first element position.
        first: SourcedValue<u32>,
        /// The original element count.
        count: SourcedValue<u32>,
        /// The parser pool's available element count.
        len: SourcedValue<DecimalU64>,
        /// The parser pool named by the original error.
        pool: OriginalErrorPool,
    },
    /// An owned structure instance reached more than once.
    OwnedTwice {
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The original target instance index.
        instance_index: SourcedValue<u32>,
    },
    /// An inline class ownership cycle.
    InlineClassCycle {
        /// The original structure index.
        struct_index: SourcedValue<u32>,
    },
    /// An original Reference whose GUID has no available target.
    DanglingReference {
        /// The original target GUID.
        guid: SourcedValue<CatalogGuid>,
    },
    /// An owned instance outside the available Record scope.
    OutsideRecord {
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The original target instance index.
        instance_index: SourcedValue<u32>,
    },
    /// An original pointer whose target type disagrees with its declaration.
    UnexpectedPointerType {
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The original declared target structure index.
        declared: SourcedValue<u32>,
    },
    /// An original Reference whose target type disagrees with its declaration.
    UnexpectedReferenceType {
        /// The original target GUID.
        guid: SourcedValue<CatalogGuid>,
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The original declared target structure index.
        declared: SourcedValue<u32>,
    },
    /// A source path beyond the decoder's representable width.
    PathTooWide {
        /// The decoder's reported representable path limit.
        limit: DecimalU64,
    },
    /// An original instance shorter than its declaration requires.
    InstanceTooShort {
        /// The original target structure index.
        struct_index: SourcedValue<u32>,
        /// The available instance byte length.
        size: SourcedValue<DecimalU64>,
        /// The declaration's required byte length.
        needed: SourcedValue<DecimalU64>,
    },
}
