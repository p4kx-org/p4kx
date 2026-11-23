// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;

use super::{PhysicalSource, PhysicalSourceRef, SourceInventory};
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_use::{SourceUseDescription, SourceUseRef};

/// Selected observations and physical ancestry from one frozen source inventory.
#[derive(Debug, Serialize)]
#[must_use]
pub struct SourceInventorySelection<'inventory> {
    sources: Vec<&'inventory PhysicalSource>,
    uses: Vec<Cow<'inventory, SourceUseDescription>>,
}

impl SourceInventorySelection<'_> {
    /// Borrows the selected physical sources in their original identifier order.
    pub fn iter_sources(&self) -> impl ExactSizeIterator<Item = &PhysicalSource> {
        self.sources.iter().copied()
    }

    /// Borrows the selected observations in their original identifier order.
    pub fn iter_uses(&self) -> impl ExactSizeIterator<Item = &SourceUseDescription> {
        self.uses.iter().map(Cow::as_ref)
    }
}

impl SourceInventory {
    /// Selects registered uses and their physical ancestry in stable identifier order.
    ///
    /// # Errors
    /// Returns an error for a foreign use reference or missing source ancestry.
    pub fn select_uses<'reference>(
        &self,
        references: impl IntoIterator<Item = &'reference SourceUseRef>,
    ) -> Result<SourceInventorySelection<'_>, SourceEvidenceError> {
        let mut uses = BTreeMap::new();
        for reference in references {
            let selected = match uses.entry(reference) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(
                    self.uses
                        .get(reference.as_str())
                        .ok_or(SourceEvidenceError::ForeignEvidence)?,
                ),
            };
            let owned = selected
                .id()
                .registered_text()
                .zip(reference.registered_text());
            if !owned.is_some_and(|(original, supplied)| Arc::ptr_eq(original, supplied)) {
                return Err(SourceEvidenceError::ForeignEvidence);
            }
        }
        let sources = self.selected_sources(uses.values().map(|row| row.source().clone()))?;
        Ok(SourceInventorySelection {
            sources,
            uses: uses.into_values().collect(),
        })
    }

    fn selected_sources(
        &self,
        references: impl IntoIterator<Item = PhysicalSourceRef>,
    ) -> Result<Vec<&PhysicalSource>, SourceEvidenceError> {
        let mut pending: BTreeSet<_> = references.into_iter().collect();
        let mut selected = BTreeMap::new();
        while let Some(reference) = pending.pop_first() {
            if let Entry::Vacant(entry) = selected.entry(reference) {
                let source = self
                    .sources
                    .find(|source| source.id().cmp(entry.key()))
                    .ok_or(SourceEvidenceError::ForeignEvidence)?;
                pending.extend(source.ancestors().iter().cloned());
                entry.insert(source);
            }
        }
        Ok(selected.into_values().collect())
    }
}
