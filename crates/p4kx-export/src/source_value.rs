// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;

use super::error::OriginalValueErrorFacts;
use super::field::SourceField;
use super::reference::{SourceRecordReference, SourceWeakReference};
use super::text::SourceText;

/// A structural reference to one original occurrence in its owner's values vector.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceValueRef {
    pub(crate) value_index: DecimalU64,
}

/// A source value and the complete error facts of that same original occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceNode<'source> {
    pub(crate) value: SourcedValue<SourcePayload<'source>>,
    pub(crate) original_error: Option<OriginalValueErrorFacts>,
}

/// Typed original contents whose scalar representation shares its node's sourced cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SourcePayload<'source> {
    Struct {
        source_type: SourcedValue<&'source str>,
        fields: Vec<SourceField<'source>>,
    },
    Array {
        elements: Vec<SourceValueRef>,
    },
    Boolean {
        value: bool,
    },
    Number {
        value: ResourceNumber,
    },
    String {
        value: SourceText<'source>,
    },
    Locale {
        value: SourceText<'source>,
    },
    Enum {
        value: &'source str,
    },
    Guid {
        value: CatalogGuid,
    },
    RecordReference {
        #[schemars(with = "CatalogRecordReference")]
        value: SourceRecordReference<'source>,
    },
    WeakReference {
        value: SourceWeakReference<'source>,
    },
}
