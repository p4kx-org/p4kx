// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUses, SourceValue, SourcedValue, Value};

use super::Fields;
use crate::CatalogError;

#[derive(Default)]
pub(in crate::catalog_v3) struct SelectionEvidence {
    uses: Vec<SourceUse>,
    missing: bool,
}

impl SelectionEvidence {
    pub(in crate::catalog_v3) fn add(&mut self, contribution: SourceUse) {
        self.uses.push(contribution);
    }

    pub(in crate::catalog_v3) fn extend(&mut self, uses: impl IntoIterator<Item = SourceUse>) {
        self.uses.extend(uses);
    }

    pub(in crate::catalog_v3) fn append(&mut self, uses: &mut Vec<SourceUse>) {
        self.uses.append(uses);
    }

    pub(in crate::catalog_v3) fn attempt(&mut self, attempt: SourceUse) {
        self.add(attempt);
        self.missing = true;
    }

    pub(super) fn value<T>(self, sources: &SourceUses, value: T) -> SourcedValue<T> {
        sources.value(SourceValue::Present(value), &self.uses, !self.missing)
    }

    pub(in crate::catalog_v3) fn into_origin(self, sources: &SourceUses) -> FieldOrigin {
        sources.origin(&self.uses, !self.missing)
    }

    pub(in crate::catalog_v3) fn retain(
        &mut self,
        fields: &Fields<'_, '_, '_>,
        selected: Option<&Value<'_>>,
    ) -> Result<bool, CatalogError> {
        let selection = fields.component_selection(selected)?;
        self.missing |= !selection.complete;
        self.extend(selection.uses);
        Ok(selection.complete)
    }
}
