// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUseRef, SourceUseRole, SourceUses};

use crate::CatalogError;

/// Contributing fields and the distinct failed reads for one variant operand.
#[derive(Clone)]
pub(crate) struct VariantEvidence {
    pub(crate) origin: FieldOrigin,
    pub(crate) failed_source_uses: Vec<SourceUseRef>,
}

impl VariantEvidence {
    pub(crate) fn from_uses(sources: &SourceUses, uses: &[SourceUse], complete: bool) -> Self {
        let mut failed_source_uses: Vec<_> = uses
            .iter()
            .filter(|used| used.role() == SourceUseRole::Attempted)
            .map(|used| used.reference().clone())
            .collect();
        failed_source_uses.sort_unstable();
        failed_source_uses.dedup();
        Self {
            origin: sources.origin(uses, complete),
            failed_source_uses,
        }
    }

    pub(crate) const fn from_origin(origin: FieldOrigin) -> Self {
        Self {
            origin,
            failed_source_uses: Vec::new(),
        }
    }

    pub(crate) fn join<'evidence>(
        sources: &SourceUses,
        inputs: impl IntoIterator<Item = &'evidence Self>,
        complete: bool,
    ) -> Result<Self, CatalogError> {
        let mut proofs = Vec::new();
        let mut failed_source_uses = Vec::new();
        for input in inputs {
            proofs.push(&input.origin);
            failed_source_uses.extend(input.failed_source_uses.iter().cloned());
        }
        failed_source_uses.sort_unstable();
        failed_source_uses.dedup();
        Ok(Self {
            origin: sources.origin_with_origins(proofs, complete)?,
            failed_source_uses,
        })
    }
}
