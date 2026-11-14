// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::{Arc, Mutex};

use p4kx_archive::{ArchiveInputOrigin, BuildManifestOrigin, EntryOrigin, ObservedEntry};

use crate::ObservedFileOrigin;
use crate::source_details::SourceCoordinateOmission;
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
    /// An exact borrowed Archive Entry without copied optional source details.
    ArchiveView(ObservedEntry<'a>),
    /// One completed whole-Archive file read bound to its actual opaque input owner.
    ArchiveFile {
        /// Observed input identity retained by the owning Archive.
        input: &'a ArchiveInputOrigin,
        /// Actual completed byte count, validated at the owning read seam.
        byte_length: u64,
    },
    /// An observed standalone file with unknown original P4K lineage.
    File(&'a ObservedFileOrigin),
    /// One completed original build-manifest read with unknown P4K lineage.
    BuildManifest(&'a BuildManifestOrigin),
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

    /// Registers an actual use whose optional coordinate capture was not admitted.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn at_opaque(
        &self,
        role: SourceUseRole,
        reason: SourceCoordinateOmission,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.at(SourceLocation::Opaque { reason }, role)
    }

    /// Compares a source with original Entry ownership without restoring authority from wire data.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn matches_entry(&self, observed: &ObservedEntry<'_>) -> Result<bool, SourceEvidenceError> {
        let registry = source_evidence_error::lock(&self.registry)?;
        Ok(registry
            .sources
            .get(&self.source)
            .and_then(|source| source.binding.as_ref())
            .is_some_and(|binding| binding.matches(&observed.binding())))
    }

    /// Returns a view of this same physical source in the requested byte domain.
    #[must_use]
    pub fn for_scope(&self, scope: InputScope) -> Self {
        Self {
            scope,
            ..self.clone()
        }
    }

    /// Returns an already observed containing Entry at its original outermost-first position.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn ancestor(&self, position: usize) -> Result<Option<Self>, SourceEvidenceError> {
        let registry = source_evidence_error::lock(&self.registry)?;
        let Some(reference) = registry
            .sources
            .get(&self.source)
            .and_then(|source| source.ancestors.get(position))
        else {
            return Ok(None);
        };
        let Some(source) = registry.sources.get(reference) else {
            return Ok(None);
        };
        let metadata =
            source
                .ancestors
                .iter()
                .fold(SourceEncryptionState::Unencrypted, |state, ancestor| {
                    state.combine(
                        registry
                            .sources
                            .get(ancestor)
                            .map_or(SourceEncryptionState::Unknown, |source| {
                                source.stored_encryption
                            }),
                    )
                });
        let payload = metadata.combine(source.stored_encryption);
        let ancestor_source = reference.clone();
        drop(registry);
        Ok(Some(Self {
            registry: Arc::clone(&self.registry),
            source: ancestor_source,
            payload,
            metadata,
            scope: self.scope,
        }))
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
