// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, SourceLocation, Value};

use super::super::tokens::{OriginalValueEdge, OriginalValuePart};
use super::OriginalValueSite;
use crate::catalog_read::original::OriginalRecordScope;

/// One retained occurrence minted only by its own admitted live value site.
pub(crate) struct OriginalValueFact<'scope, 'read, 'source> {
    scope: &'scope OriginalRecordScope<'read, 'source>,
    field: &'read Field<'source>,
    value: &'read Value<'source>,
    ordinal: u64,
    declaration_ordinal: u64,
    owner_type: &'source str,
    parent: Option<u64>,
    edge: OriginalValueEdge,
}

impl<'scope, 'read, 'source> OriginalValueFact<'scope, 'read, 'source> {
    pub(super) const fn capture(site: &OriginalValueSite<'_, 'scope, 'read, 'source>) -> Self {
        Self {
            scope: site.scope,
            field: site.field(),
            value: site.value(),
            ordinal: site.ordinal(),
            declaration_ordinal: site.visit.declaration_ordinal(),
            owner_type: site.owner_type(),
            parent: site.parent_ordinal(),
            edge: site.edge(),
        }
    }

    pub(crate) const fn value(&self) -> &'read Value<'source> {
        self.value
    }

    pub(crate) const fn field(&self) -> &'read Field<'source> {
        self.field
    }

    pub(crate) const fn owner_type(&self) -> &'source str {
        self.owner_type
    }

    pub(crate) const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub(crate) const fn parent_ordinal(&self) -> Option<u64> {
        self.parent
    }

    pub(crate) const fn edge(&self) -> OriginalValueEdge {
        self.edge
    }

    pub(in super::super::super) const fn scope(
        &self,
    ) -> &'scope OriginalRecordScope<'read, 'source> {
        self.scope
    }

    /// Rejects a projection input other than this fact's genuinely admitted source.
    pub(crate) fn check_input(
        &self,
        input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
    ) -> Result<(), crate::CatalogError> {
        let context = input.record_context(self.scope.record())?;
        if !std::ptr::eq(context.dataforge(), self.scope.dataforge())
            || !context.sources().is_same_inventory(self.scope.sources())
        {
            return Err(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(crate) fn location(&self, part: OriginalValuePart) -> SourceLocation {
        let ordinal = if part.is_declaration() {
            self.declaration_ordinal
        } else {
            self.ordinal
        };
        SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![u64::from(self.scope.record().index()), ordinal],
            field: part.label().to_owned(),
        }
    }
}
