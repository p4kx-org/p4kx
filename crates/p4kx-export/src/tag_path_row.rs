// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use p4kx_extraction::{CancelToken, SourceValue, SourcedValue};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use super::path_capture::{NodeInputs, PathReceipts, PathUse};
use crate::CatalogError;
use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One selected path and its shared original receipts.
pub(crate) struct TagPath {
    receipts: Arc<PathReceipts>,
    position: usize,
}

impl TagPath {
    pub(super) fn rows(
        receipts: &Arc<PathReceipts>,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<Self>, CatalogError> {
        (0..receipts.len())
            .map(|position| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                Ok(Self {
                    receipts: Arc::clone(receipts),
                    position,
                })
            })
            .collect()
    }

    fn row<'receipt>(
        &'receipt self,
        node: &'receipt NodeInputs,
        text: String,
    ) -> Result<TagPathRow<'receipt>, ExportAccountingError> {
        let uses = &self.receipts.uses;
        let construction = self.receipts.lineage(self.position, PathUse::Construction);
        let path = uses.value(
            SourceValue::Present(text),
            &construction,
            node.path_complete,
        );
        drop(construction);
        let mut selection = self.receipts.lineage(self.position, PathUse::Topology);
        let depth = uses.value(SourceValue::Present(node.depth), &selection, true);
        let listed = depth.clone().map(|_| node.listed);
        let parent = match &node.parent {
            Some(parent) => {
                selection.push(parent.payload.clone());
                let parent_node = self
                    .receipts
                    .get(parent.position as usize)
                    .ok_or(ExportAccountingError::ConflictingTagTrace)?;
                TagPathParent::Tag {
                    guid: &parent_node.guid_cell,
                    record_index: &parent_node.record_index_cell,
                }
            }
            None => TagPathParent::Root,
        };
        Ok(TagPathRow {
            guid: &node.guid_cell,
            record_index: &node.record_index_cell,
            path,
            depth,
            listed,
            parent: uses.value(SourceValue::Present(parent), &selection, true),
        })
    }
}

impl Debug for TagPath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TagPath")
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl Serialize for TagPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let node = self
            .receipts
            .get(self.position)
            .ok_or_else(|| S::Error::custom(ExportAccountingError::ConflictingTagTrace))?;
        let tag = self
            .receipts
            .tree
            .at(self.position)
            .ok_or_else(|| S::Error::custom(ExportAccountingError::ConflictingTagTrace))?;
        self.row(node, tag.path_view().to_string())
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl JsonSchema for TagPath {
    fn inline_schema() -> bool {
        TagPathRow::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        TagPathRow::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::TagPath"))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        TagPathRow::json_schema(generator)
    }
}

/// The selected parent of a first-depth-first convenience path.
#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum TagPathParent<'receipt> {
    Root,
    Tag {
        guid: &'receipt SourcedValue<CatalogGuid>,
        record_index: &'receipt SourcedValue<u32>,
    },
}

/// One first-selected convenience path with cycles cut. Authored edges remain authoritative.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TagPath", transform = every_field_required)]
struct TagPathRow<'receipt> {
    guid: &'receipt SourcedValue<CatalogGuid>,
    record_index: &'receipt SourcedValue<u32>,
    path: SourcedValue<String>,
    depth: SourcedValue<u32>,
    listed: SourcedValue<bool>,
    parent: SourcedValue<TagPathParent<'receipt>>,
}
