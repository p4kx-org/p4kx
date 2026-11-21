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
pub struct SourceValueRef {
    pub(crate) value_index: DecimalU64,
}

impl SourceValueRef {
    /// Returns the occurrence's position in its owner's value arena.
    #[must_use]
    pub const fn value_index(self) -> u64 {
        self.value_index.get()
    }
}

/// A source value and the complete error facts of that same original occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SourceNode<'source> {
    pub(crate) value: SourcedValue<SourcePayload<'source>>,
    pub(crate) original_error: Option<Box<OriginalValueErrorFacts>>,
}

impl<'source> SourceNode<'source> {
    /// Borrows the original state, typed contents and source evidence.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<SourcePayload<'source>> {
        &self.value
    }

    /// Borrows the original decoder error when this occurrence was unreadable.
    #[must_use]
    pub fn original_error(&self) -> Option<&OriginalValueErrorFacts> {
        self.original_error.as_deref()
    }
}

/// Typed original contents whose scalar representation shares its node's sourced cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum SourcePayload<'source> {
    /// An owned structure with its concrete type and ordered field occurrences.
    Struct {
        /// The concrete source structure type with its evidence.
        source_type: SourcedValue<&'source str>,
        /// Fields in their original declaration order.
        fields: Vec<SourceField<'source>>,
    },
    /// An ordered list whose links belong to the same owner's arena.
    Array {
        /// Original element positions, including null and unreadable occurrences.
        elements: Vec<SourceValueRef>,
    },
    /// An original Boolean.
    Boolean {
        /// The decoded value.
        value: bool,
    },
    /// An exact numeric value with its original storage kind.
    Number {
        /// The original integer width or IEEE representation.
        value: ResourceNumber,
    },
    /// An original string with its text domain.
    String {
        /// The authored text.
        value: SourceText<'source>,
    },
    /// An original localization key with its text domain.
    Locale {
        /// The authored localization key.
        value: SourceText<'source>,
    },
    /// An original enum option.
    Enum {
        /// The decoded option text.
        value: &'source str,
    },
    /// An original GUID value.
    Guid {
        /// The stored GUID.
        value: CatalogGuid,
    },
    /// An original Reference with its captured target header and resolution state.
    RecordReference {
        /// The physically resolved or unresolved target.
        #[schemars(with = "CatalogRecordReference")]
        value: Box<SourceRecordReference<'source>>,
    },
    /// An original weak pointer to a Record or an owned instance.
    WeakReference {
        /// The original owner, coordinate and target facts.
        value: Box<SourceWeakReference<'source>>,
    },
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{SourceNode, SourcePayload, SourceRecordReference, SourceWeakReference};

    #[test]
    fn scalar_nodes_store_reference_payloads_indirectly() {
        assert!(
            size_of::<SourcePayload<'_>>() < size_of::<SourceRecordReference<'_>>(),
            "scalar payloads must not allocate space for resolved references"
        );
        assert!(
            size_of::<SourceNode<'_>>() < size_of::<SourceWeakReference<'_>>(),
            "an ordinary node, including error and origin storage, must fit below a weak reference"
        );
    }
}
