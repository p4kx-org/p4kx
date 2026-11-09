// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::header::TagHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One physical Tag definition, independently of convenience-tree cuts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tag_name: SourcedValue<&'source str>,
    pub(crate) legacy_guid: SourcedValue<u32>,
    pub(crate) children: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// One `TagDatabase`'s original root array with every stored occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagRoots<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// One selected Entity's root GUID-tag declaration, separate from `AttachDef` words.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EntityTags<'source> {
    #[serde(flatten)]
    pub(crate) source: TagHeader<'source>,
    pub(crate) tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// The selected parent of a first-depth-first convenience path.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum TagPathParent {
    Root,
    Tag {
        guid: SourcedValue<CatalogGuid>,
        record_index: SourcedValue<u32>,
    },
}

/// One first-selected, cycle-cut convenience path; authored edges remain authoritative.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagPath {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) path: SourcedValue<String>,
    pub(crate) depth: SourcedValue<u32>,
    pub(crate) listed: SourcedValue<bool>,
    pub(crate) parent: SourcedValue<TagPathParent>,
}

/// Prepared rows and stable source keys used for accounting and sorting.
#[derive(Debug, Default)]
pub(crate) struct TagRows<'source> {
    pub(crate) definitions: Vec<TagDefinition<'source>>,
    pub(crate) roots: Vec<TagRoots<'source>>,
    pub(crate) paths: Vec<TagPath>,
    pub(crate) entity_tags: Vec<EntityTags<'source>>,
}

impl TagRows<'_> {
    pub(crate) const fn count(&self) -> usize {
        self.definitions.len()
    }

    pub(crate) fn order_entities(&mut self) {
        self.entity_tags.sort_by_key(|row| {
            (
                CatalogGuid(row.source.physical.guid).text(),
                row.source.physical.record_index,
            )
        });
    }
}
