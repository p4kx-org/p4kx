// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::header::TagHeader;
use super::path_row::TagPath;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::schema_fields::every_field_required;

/// One physical Tag definition, independently of convenience-tree cuts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct TagDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tag_name: SourcedValue<&'source str>,
    pub(crate) legacy_guid: SourcedValue<u32>,
    pub(crate) children: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

impl<'source> TagDefinition<'source> {
    /// Returns the physical Tag Record's original header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source.header
    }

    /// Returns the authored name.
    #[must_use]
    pub const fn tag_name(&self) -> &SourcedValue<&'source str> {
        &self.tag_name
    }

    /// Returns the original legacy identity.
    #[must_use]
    pub const fn legacy_guid(&self) -> &SourcedValue<u32> {
        &self.legacy_guid
    }

    /// Returns the authored child slots in array order.
    #[must_use]
    pub const fn children(&self) -> &SourcedValue<Vec<SourcedValue<CatalogRecordReference>>> {
        &self.children
    }

    pub(crate) fn order_key(&self) -> (GuidText, u32) {
        (
            CatalogGuid(self.source.physical.guid).text(),
            self.source.physical.record_index,
        )
    }
}

/// One `TagDatabase`'s original root array with every stored occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct TagRoots<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

impl<'source> TagRoots<'source> {
    /// Returns the physical `TagDatabase` Record's original header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source.header
    }

    /// Returns the authored root slots in array order.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<Vec<SourcedValue<CatalogRecordReference>>> {
        &self.tags
    }

    pub(crate) const fn order_key(&self) -> u32 {
        self.source.physical.record_index
    }
}

/// One selected Entity's root GUID-tag declaration, separate from `AttachDef` words.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EntityTags<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

impl<'source> EntityTags<'source> {
    /// Returns the original Entity's physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source.header
    }

    /// Returns the authored GUID-tag occurrences in their original array order.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<Vec<SourcedValue<CatalogRecordReference>>> {
        &self.tags
    }

    pub(crate) fn order_key(&self) -> (GuidText, u32) {
        (
            CatalogGuid(self.source.physical.guid).text(),
            self.source.physical.record_index,
        )
    }
}

/// Prepared rows and stable source keys used for accounting and sorting.
#[derive(Debug, Default)]
pub(crate) struct TagRows<'source> {
    pub(crate) definitions: Vec<TagDefinition<'source>>,
    pub(crate) roots: Vec<TagRoots<'source>>,
    pub(crate) paths: Vec<TagPath>,
}

impl TagRows<'_> {
    pub(crate) const fn count(&self) -> usize {
        self.definitions.len()
    }
}
