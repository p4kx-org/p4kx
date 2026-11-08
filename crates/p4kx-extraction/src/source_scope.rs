// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::{Arc, Mutex};

use p4kx_archive::EntryOrigin;

use crate::DataForgeFileOrigin;
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::source_registry::SourceRegistry;
use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};

/// Actual observed input; no path, matching bytes or key flag can replace this binding.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SourceInput<'a> {
    /// An observed Archive Entry and its containing Entries.
    Archive(&'a EntryOrigin),
    /// An observed standalone file with unknown original P4K lineage.
    File(&'a DataForgeFileOrigin),
    /// An input without trusted physical binding.
    Unknown,
}

/// A shared physical-source scope used when binding individual data values.
#[derive(Clone)]
pub struct SourceScope {
    pub(crate) registry: Arc<Mutex<SourceRegistry>>,
    pub(crate) source: PhysicalSourceRef,
    pub(crate) payload: SourceEncryptionState,
    pub(crate) metadata: SourceEncryptionState,
    pub(crate) scope: InputScope,
}

impl SourceScope {
    /// Registers the exact observed use without changing the physical-source identity.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn at(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        Ok(source_evidence_error::lock(&self.registry)?.register_use(
            &self.source,
            self.scope,
            location,
            role,
            self.encryption(),
        ))
    }

    /// Returns a view of this same physical source in the requested byte domain.
    #[must_use]
    pub fn for_scope(&self, scope: InputScope) -> Self {
        Self {
            scope,
            ..self.clone()
        }
    }

    /// Returns this scope's physical-source reference.
    #[must_use]
    pub const fn source_ref(&self) -> &PhysicalSourceRef {
        &self.source
    }

    pub(crate) const fn encryption(&self) -> SourceEncryptionState {
        match self.scope {
            InputScope::Payload => self.payload,
            InputScope::ArchiveDirectoryMetadata | InputScope::FileMetadata => self.metadata,
        }
    }
}

impl fmt::Debug for SourceScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceScope")
            .field("source", &self.source)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
