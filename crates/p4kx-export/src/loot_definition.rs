// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

use crate::source_value_catalog::{SourceField, SourceNode, SourceValueRef};

/// Complete ordered source fields and values of one physical Loot Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LootDefinition<'source> {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_path: &'source str,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(skip)]
    pub(crate) incomplete_fields: bool,
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) fields: Vec<SourceField<'source>>,
    pub(crate) values: Vec<SourceNode<'source>>,
}

impl<'source> LootDefinition<'source> {
    /// Borrows the physical Record identity and independently sourced metadata.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the ordered original root fields.
    #[must_use]
    pub fn fields(&self) -> &[SourceField<'source>] {
        &self.fields
    }

    /// Borrows the complete physical owner's flat values.
    #[must_use]
    pub fn values(&self) -> &[SourceNode<'source>] {
        &self.values
    }

    /// Resolves a reference from this physical owner's arena.
    #[must_use]
    pub fn value(&self, reference: SourceValueRef) -> Option<&SourceNode<'source>> {
        self.values
            .get(usize::try_from(reference.value_index()).ok()?)
    }
}
