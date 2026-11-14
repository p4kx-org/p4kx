// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use p4kx_archive::Archive;

#[path = "source_use_batches.rs"]
mod batches;

#[path = "source_origin_union.rs"]
mod union;

#[path = "source_inventory_freeze.rs"]
mod freeze;
pub use self::freeze::SourceInventoryFreezeError;

use crate::DecimalU64;
use crate::field_origin::{FieldOrigin, SharedFieldOrigin};
use crate::source_details::{SourceCoordinateOmission, SourceDetails};
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceEncryptionState;
use crate::source_registry::SourceRegistry;
use crate::source_scope::{SourceInput, SourceScope};
use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};
use crate::source_value::{SourceValue, SourcedValue};

/// Per-preparation physical source and use inventory shared by parallel field projections.
#[derive(Clone)]
pub struct SourceUses {
    registry: Arc<Mutex<SourceRegistry>>,
    identity: Arc<()>,
}

impl SourceUses {
    /// Starts an empty inventory; inputs are observed in a deterministic preparation order.
    #[must_use]
    pub fn new() -> Self {
        Self::with_registry(SourceRegistry::new)
    }

    /// Starts an inventory with short, document-local source-use references.
    /// Register uses in deterministic order for repeatable serialized references.
    #[must_use]
    pub fn with_compact_use_refs() -> Self {
        Self::with_registry(SourceRegistry::with_compact_use_refs)
    }

    fn with_registry(make: impl FnOnce(Arc<()>) -> SourceRegistry) -> Self {
        let identity = Arc::new(());
        let registry = Arc::new(Mutex::new(make(Arc::clone(&identity))));
        Self { registry, identity }
    }

    /// Retains a complete survey of the admitted paths in this root Archive directory.
    /// Call after consulting every admitted path. A nested Archive returns no root survey.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn observe_root_directory_paths(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory(archive, |count| {
            Ok(SourceLocation::ArchiveDirectoryPaths {
                start_entry_index: DecimalU64::new(0),
                end_entry_index: count,
            })
        })
    }

    /// Retains the actual folder tree path range of a root Archive.
    /// Pass positions from the owning root Archive's `EntryTree`.
    ///
    /// # Errors
    /// Returns an error for a reversed or out-of-bounds range or interrupted evidence registry.
    pub fn observe_root_directory_tree_paths(
        &self,
        archive: &Archive,
        paths: Range<u32>,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory(archive, |count| {
            let start = u64::from(paths.start);
            let end = u64::from(paths.end);
            if start > end || end > count.get() {
                return Err(SourceEvidenceError::InvalidDirectoryRange);
            }
            Ok(SourceLocation::ArchiveDirectoryTreePaths {
                start_entry_index: DecimalU64::new(start),
                end_entry_index: DecimalU64::new(end),
            })
        })
    }

    fn observe_root_directory(
        &self,
        archive: &Archive,
        location: impl FnOnce(DecimalU64) -> Result<SourceLocation, SourceEvidenceError>,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        let Some(scope) = self.root_directory_scope(archive)? else {
            return Ok(None);
        };
        let count = DecimalU64::new(archive.entries().len() as u64);
        Ok(Some(scope.at(location(count)?, SourceUseRole::Selector)?))
    }

    pub(crate) fn root_directory_scope(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceScope>, SourceEvidenceError> {
        let Some(count) = archive.root_directory_entry_count() else {
            return Ok(None);
        };
        let source = source_evidence_error::lock(&self.registry)?
            .root_directory(archive.input_origin(), DecimalU64::new(count as u64));
        Ok(Some(SourceScope {
            registry: Arc::clone(&self.registry),
            source,
            payload: SourceEncryptionState::Unknown,
            metadata: SourceEncryptionState::Unencrypted,
            scope: InputScope::ArchiveDirectoryMetadata,
        }))
    }

    /// Retains the actual input once and returns a shared payload or metadata scope.
    /// Observe inputs in fixed order before projecting their values in parallel.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn observe(
        &self,
        input: SourceInput<'_>,
        scope: InputScope,
    ) -> Result<SourceScope, SourceEvidenceError> {
        self.observe_with_details(input, scope, SourceDetails::Full)
    }

    /// Retains original input proof independently of optional source detail capture.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn observe_with_details(
        &self,
        input: SourceInput<'_>,
        scope: InputScope,
        details: SourceDetails,
    ) -> Result<SourceScope, SourceEvidenceError> {
        let source = source_evidence_error::lock(&self.registry)?.observe(input, details);
        Ok(SourceScope {
            registry: Arc::clone(&self.registry),
            source: source.id,
            payload: source.payload,
            metadata: source.metadata,
            scope,
        })
    }

    /// Retains an actual registered use while omitting only its optional source coordinate.
    ///
    /// # Errors
    /// Returns an error for a foreign owner or interrupted evidence registry.
    pub fn opaque_use(
        &self,
        actual: &SourceUse,
        reason: SourceCoordinateOmission,
    ) -> Result<SourceUse, SourceEvidenceError> {
        if !Arc::ptr_eq(&actual.identity, &self.identity) {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let mut registry = source_evidence_error::lock(&self.registry)?;
        let registered = registry
            .uses
            .get(actual.reference())
            .is_some_and(|row| Arc::ptr_eq(row, &actual.description));
        if !registered {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let original = actual.description();
        Ok(registry.register_use(
            original.source(),
            original.scope(),
            SourceLocation::Opaque { reason },
            original.role(),
            actual.encryption(),
        ))
    }

    /// Returns whether two preparation handles share the same original evidence authority.
    #[must_use]
    pub fn is_same_inventory(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }

    /// Binds a typed value to its actual retained payload, selector and conversion uses.
    /// Foreign-inventory contributions make the set incomplete and are never misattributed.
    #[must_use]
    pub fn value<T>(
        &self,
        value: SourceValue<T>,
        uses: &[SourceUse],
        source_set_complete: bool,
    ) -> SourcedValue<T> {
        SourcedValue::new(value, self.origin(uses, source_set_complete))
    }

    /// Binds navigation to its actual contributing inputs without duplicating a game value.
    #[must_use]
    pub fn origin(&self, uses: &[SourceUse], source_set_complete: bool) -> FieldOrigin {
        let (own, complete) = self.owned_uses(uses, source_set_complete);
        FieldOrigin::new(Arc::clone(&self.identity), own, complete)
    }

    /// Captures a contributor set shared by values derived from the same source survey.
    #[must_use]
    pub fn shared_origin(
        &self,
        uses: &[SourceUse],
        source_set_complete: bool,
    ) -> SharedFieldOrigin {
        let (own, complete) = self.owned_uses(uses, source_set_complete);
        SharedFieldOrigin::new(Arc::clone(&self.identity), own, complete)
    }

    /// Binds a value to the union of a shared original contributor set and its own uses.
    ///
    /// # Errors
    /// Returns an error when the shared origin belongs to another inventory.
    pub fn value_with_shared_origin<T>(
        &self,
        value: SourceValue<T>,
        shared: &SharedFieldOrigin,
        uses: &[SourceUse],
        source_set_complete: bool,
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let origin = self.origin_with_shared(shared, uses, source_set_complete)?;
        Ok(SourcedValue::new(value, origin))
    }

    /// Binds an output address to its shared and field-specific contributors.
    ///
    /// # Errors
    /// Returns an error when the shared origin belongs to another inventory.
    pub fn origin_with_shared(
        &self,
        shared: &SharedFieldOrigin,
        uses: &[SourceUse],
        source_set_complete: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        if !shared.belongs_to(&self.identity) {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let (own, complete) = self.owned_uses(uses, source_set_complete);
        Ok(FieldOrigin::with_shared(shared, own, complete))
    }

    fn owned_uses<'a>(
        &'a self,
        uses: &'a [SourceUse],
        source_set_complete: bool,
    ) -> (impl Iterator<Item = &'a SourceUse> + 'a, bool) {
        let foreign = uses.iter().any(|source| {
            source.role().contributes() && !Arc::ptr_eq(&source.identity, &self.identity)
        });
        let own = uses
            .iter()
            .filter(move |source| Arc::ptr_eq(&source.identity, &self.identity));
        (own, source_set_complete && !foreign)
    }
}

impl Default for SourceUses {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SourceUses {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("SourceUses").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::SourceUses;
    use crate::{InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUseRole};

    #[test]
    fn compact_use_refs_reuse_only_the_same_original_use() -> Result<(), SourceEvidenceError> {
        let sources = SourceUses::with_compact_use_refs();
        let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
        let at = |index| {
            scope.at(
                SourceLocation::Subresource {
                    namespace: "synthetic".into(),
                    indices: vec![index],
                    field: "same".into(),
                },
                SourceUseRole::Payload,
            )
        };
        let first = at(0)?;
        let repeated = at(0)?;
        let second = at(1)?;
        assert_eq!(
            first.reference().as_str(),
            "u0",
            "short document-local reference"
        );
        assert_eq!(first.reference(), repeated.reference(), "same original use");
        assert_eq!(
            second.reference().as_str(),
            "u1",
            "a different original ordinal"
        );
        let inventory = sources.freeze()?;
        assert_eq!(
            inventory.uses().len(),
            2,
            "the two references resolve uniquely"
        );
        Ok(())
    }

    #[test]
    fn interrupted_evidence_rejects_observation_registration_and_freeze() {
        let sources = SourceUses::new();
        let scope = sources
            .observe(SourceInput::Unknown, InputScope::Payload)
            .expect("initial scope");
        let unwound = std::thread::scope(|threads| {
            threads
                .spawn(|| {
                    let _guard = sources.registry.lock().expect("initial registry lock");
                    panic!("synthetic interrupted source-evidence mutation");
                })
                .join()
                .is_err()
        });
        assert!(
            unwound && sources.registry.is_poisoned(),
            "fixture poisons the real registry"
        );
        assert!(
            matches!(
                sources.observe(SourceInput::Unknown, InputScope::Payload),
                Err(SourceEvidenceError::PoisonedEvidence)
            ),
            "no input may enter poisoned evidence"
        );
        assert!(
            matches!(
                scope.at(
                    SourceLocation::Metadata {
                        field: "path".into()
                    },
                    SourceUseRole::Payload
                ),
                Err(SourceEvidenceError::PoisonedEvidence)
            ),
            "no source use may enter poisoned evidence"
        );
        assert!(
            matches!(sources.freeze(), Err(SourceEvidenceError::PoisonedEvidence)),
            "poisoned evidence cannot become a published inventory"
        );
    }
}
