// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{SourceUse, SourceUses, SourceValue};

use super::{OriginalTagPath, PathReceipts};
use crate::CatalogError;
use crate::catalog_v3::tag_catalog::dependent_origins::{ContextualSourcedValue, TagInputs};

/// A selected Tag value and the actual occurrence-local observations that selected it.
#[derive(Clone)]
pub(crate) struct TagDatum {
    value: SourceValue<OriginalTagPath>,
    inputs: TagInputs,
}

pub(crate) type ContextualTagPath = ContextualSourcedValue<OriginalTagPath>;

impl TagDatum {
    pub(crate) const fn local(
        value: SourceValue<OriginalTagPath>,
        inputs: Vec<SourceUse>,
        complete: bool,
    ) -> Self {
        Self {
            value,
            inputs: TagInputs::local(inputs, complete),
        }
    }

    pub(super) fn selected(
        value: OriginalTagPath,
        receipts: &Arc<PathReceipts>,
        position: usize,
        complete: bool,
    ) -> Self {
        Self {
            value: SourceValue::Present(value),
            inputs: TagInputs::selected(receipts, position, complete),
        }
    }

    pub(crate) fn contribute(&mut self, inputs: Vec<SourceUse>) {
        self.inputs.retain(inputs, true);
    }

    pub(crate) fn split(self) -> (SourceValue<OriginalTagPath>, TagInputs) {
        (self.value, self.inputs)
    }

    pub(crate) fn into_cell(self, sources: &SourceUses) -> Result<ContextualTagPath, CatalogError> {
        self.inputs.bind(self.value, sources)
    }
}
