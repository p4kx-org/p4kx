// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, SourceLocation, SourceUse, SourceUseRole, SourceValue, TagTree,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::path_capture::{PathCapture, PathReceipts};
use super::super::path_row::TagPath;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalReferenceObservation;
use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;

#[path = "original_tag_path.rs"]
mod path;

pub(crate) use path::OriginalTagPath;

#[path = "tag_dependent_datum.rs"]
mod datum;

pub(crate) use datum::{ContextualTagPath, TagDatum};

#[path = "tag_name_selector.rs"]
mod name;
pub(crate) use name::TagNameSelector;

/// The same immutable tree and original node receipts selected by the Tag build.
#[derive(Clone)]
pub(crate) struct CapturedTagTree {
    tree: Arc<TagTree>,
}

impl CapturedTagTree {
    pub(crate) fn tree(&self) -> &TagTree {
        &self.tree
    }
}

/// The one actual Tag build and its still-bound input.
pub(crate) struct CapturedTags<'context, 'source> {
    input: &'context OriginalCatalogInput<'context, 'source>,
    tree: CapturedTagTree,
    paths: Arc<PathReceipts>,
    absence: Vec<SourceUse>,
    negative_complete: bool,
}

#[derive(Clone)]
pub(crate) enum TagPathLookup<T = TagDatum> {
    Ready { meaning: TagPathMeaning, value: T },
    Unavailable { value: T },
}

impl<T> TagPathLookup<T> {
    pub(crate) fn map<U>(self, value_of: impl FnOnce(T) -> U) -> TagPathLookup<U> {
        match self {
            Self::Ready { meaning, value } => TagPathLookup::Ready {
                meaning,
                value: value_of(value),
            },
            Self::Unavailable { value } => TagPathLookup::Unavailable {
                value: value_of(value),
            },
        }
    }
}

/// The lookup's interpretation independently of pointer presence and readability.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum TagPathMeaning {
    Path,
    GuidFallback,
    Inapplicable,
    Unresolved,
}

impl<'context, 'source> CapturedTags<'context, 'source> {
    pub(super) fn finish(
        input: &'context OriginalCatalogInput<'context, 'source>,
        tree: TagTree,
        capture: PathCapture,
        inventory_complete: bool,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let tree = Arc::new(tree);
        let paths = capture.finish(Arc::clone(&tree), &input.origins().uses, cancel)?;
        let negative_complete = inventory_complete
            && tree.issues().unclassified() == 0
            && tree.issues().unreadable() == 0;
        let inventory = input.origins().dataforge.at(
            SourceLocation::Subresource {
                namespace: "dataforgeTagInventory".to_owned(),
                indices: Vec::new(),
                field: "$records".to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        Ok(Self {
            input,
            tree: CapturedTagTree { tree },
            paths,
            absence: vec![inventory],
            negative_complete,
        })
    }

    pub(crate) fn tree(&self) -> &TagTree {
        self.tree.tree()
    }

    pub(crate) fn tree_owner(&self) -> CapturedTagTree {
        self.tree.clone()
    }

    pub(super) fn path_rows(
        &self,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<TagPath>, CatalogError> {
        TagPath::rows(&self.paths, cancel)
    }

    pub(crate) fn lookup(
        &self,
        observed: &OriginalReferenceObservation<'_, '_, '_, 'source>,
    ) -> Result<TagPathLookup, CatalogError> {
        observed.caller().check_input(self.input)?;
        let reference = observed.reference();
        if reference.location().is_some() {
            return Ok(TagPathLookup::Ready {
                value: TagDatum::local(SourceValue::Absent, Vec::new(), true),
                meaning: TagPathMeaning::Inapplicable,
            });
        }
        let tree = &self.tree.tree;
        let Some(tag) = tree.get(reference.guid()) else {
            if !self.negative_complete {
                return Ok(TagPathLookup::Unavailable {
                    value: TagDatum::local(SourceValue::Unreadable, self.absence.clone(), false),
                });
            }
            return Ok(TagPathLookup::Ready {
                value: TagDatum::local(
                    SourceValue::Present(OriginalTagPath::fallback(CatalogGuid(reference.guid()))),
                    self.absence.clone(),
                    true,
                ),
                meaning: TagPathMeaning::GuidFallback,
            });
        };
        let node = self
            .paths
            .get(tag.position())
            .ok_or(ExportAccountingError::ConflictingTagTrace)?;
        if node.guid != tag.guid() {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        Ok(TagPathLookup::Ready {
            value: TagDatum::selected(
                OriginalTagPath::selected(tree, tag.position())?,
                &self.paths,
                tag.position(),
                node.path_complete,
            ),
            meaning: TagPathMeaning::Path,
        })
    }
}
