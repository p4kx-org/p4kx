// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::sync::Arc;

use crate::origin_companion;
use crate::source_evidence_error;
use crate::{OriginCompanionRow, SourceUse};

use super::{SourceRegistry, SourceUses};

impl SourceUses {
    pub(crate) fn with_batched_use_refs() -> Self {
        Self::with_registry(SourceRegistry::with_batched_use_refs)
    }

    pub(crate) fn write_use_batch(&self, writer: &mut impl Write) -> io::Result<u64> {
        let uses = source_evidence_error::lock(&self.registry)
            .map_err(io::Error::other)?
            .take_use_batch()?;
        for source_use in uses.values() {
            origin_companion::row(writer, &OriginCompanionRow::SourceUse { source_use })?;
        }
        Ok(uses.len() as u64)
    }

    pub(crate) fn accepts_use_batch(&self, uses: &[SourceUse]) -> io::Result<bool> {
        let registry = source_evidence_error::lock(&self.registry).map_err(io::Error::other)?;
        Ok(uses.iter().all(|used| {
            registry
                .uses
                .get(used.reference())
                .is_some_and(|row| Arc::ptr_eq(row, &used.description))
        }))
    }
}
