// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use p4kx_extraction::TagTree;

/// Shared immutable lookup text selected from the actual Tag build.
#[derive(Clone)]
pub(crate) struct OriginalTagPath {
    value: Text,
}

#[derive(Clone)]
enum Text {
    Path { tree: Arc<TagTree>, position: usize },
    Guid(CatalogGuid),
}

impl OriginalTagPath {
    pub(super) fn selected(
        tree: &Arc<TagTree>,
        position: usize,
    ) -> Result<Self, ExportAccountingError> {
        if tree.at(position).is_none() {
            return Err(ExportAccountingError::ConflictingTagTrace);
        }
        Ok(Self {
            value: Text::Path {
                tree: Arc::clone(tree),
                position,
            },
        })
    }

    pub(super) const fn fallback(guid: CatalogGuid) -> Self {
        Self {
            value: Text::Guid(guid),
        }
    }
}

impl fmt::Debug for OriginalTagPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OriginalTagPath")
            .finish_non_exhaustive()
    }
}

impl Serialize for OriginalTagPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        match &self.value {
            Text::Guid(guid) => guid.serialize(serializer),
            Text::Path { tree, position } => {
                let tag = tree
                    .at(*position)
                    .ok_or_else(|| S::Error::custom(ExportAccountingError::ConflictingTagTrace))?;
                serializer.collect_str(&tag.path_view())
            }
        }
    }
}

impl JsonSchema for OriginalTagPath {
    fn inline_schema() -> bool {
        String::inline_schema()
    }
    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }
    fn schema_id() -> Cow<'static, str> {
        String::schema_id()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }
}
