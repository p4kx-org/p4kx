// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;

use super::{CatalogProjection, CatalogTextRegistry, RegistryState};
use crate::{CatalogError, ExportAccountingError};

impl<'source> CatalogProjection<'_, 'source> {
    /// Stages text registrations against the same observed inputs.
    pub(crate) const fn with_text_registry<'staged>(
        &'staged self,
        registry: &'staged CatalogTextRegistry<'source>,
    ) -> CatalogProjection<'staged, 'source> {
        CatalogProjection {
            texts: self.texts,
            original: self.original,
            registry,
            owner: self.owner,
        }
    }

    /// Moves staged text registrations into this projection's registry.
    pub(crate) fn commit_text_registrations(
        &self,
        staged: &CatalogProjection<'_, 'source>,
    ) -> Result<(), CatalogError> {
        if std::ptr::eq(self.registry, staged.registry)
            || !std::ptr::eq(self.texts, staged.texts)
            || !self
                .origins()
                .uses
                .is_same_inventory(&staged.origins().uses)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }

        let incoming = std::mem::take(&mut *crate::export_evidence_lock::lock(
            &staged.registry.state,
        )?);
        crate::export_evidence_lock::lock(&self.registry.state)?.merge(incoming);
        Ok(())
    }
}

impl RegistryState<'_> {
    fn merge(&mut self, incoming: Self) {
        for (key, row) in incoming.pending {
            match self.pending.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(row);
                }
                Entry::Occupied(mut entry) => {
                    let current = entry.get_mut();
                    current.payloads.extend(row.payloads);
                    current.selectors.extend(row.selectors);
                }
            }
        }
        self.coverage.merge(incoming.coverage);
    }
}
