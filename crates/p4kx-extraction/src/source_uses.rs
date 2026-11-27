// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::{Arc, Mutex};

#[path = "source_directory_observations.rs"]
mod directory;

#[path = "source_use_batches.rs"]
mod batches;

#[path = "source_origin_union.rs"]
mod union;

#[path = "source_inventory_freeze.rs"]
mod freeze;
pub use self::freeze::SourceInventoryFreezeError;

use crate::ProvenanceMode;
use crate::field_origin::{FieldOrigin, SharedFieldOrigin};
use crate::source_details::{SourceCoordinateOmission, SourceDetails};
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_registry::SourceRegistry;
use crate::source_scope::{SourceInput, SourceScope};
use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};
use crate::source_value::{SourceValue, SourcedValue};

/// Per-preparation physical source and use inventory shared by parallel field projections.
#[derive(Clone)]
pub struct SourceUses {
    registry: Arc<Mutex<SourceRegistry>>,
    identity: Arc<()>,
    mode: ProvenanceMode,
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

    /// Starts an inventory with compact source-use references independent of use observation order.
    /// Register physical inputs in deterministic order for repeatable serialized references.
    #[must_use]
    pub fn with_compact_coordinates() -> Self {
        Self::with_registry(SourceRegistry::with_compact_coordinates)
    }

    /// Creates a compact evidence capture for the requested provenance profile.
    #[must_use]
    pub fn with_provenance_mode(mode: ProvenanceMode) -> Self {
        let identity = Arc::new(());
        let registry = Arc::new(Mutex::new(SourceRegistry::with_provenance_mode(
            Arc::clone(&identity),
            mode,
        )));
        Self {
            registry,
            identity,
            mode,
        }
    }

    /// Reports whether this capture retains a detailed source graph.
    #[must_use]
    pub const fn provenance_mode(&self) -> ProvenanceMode {
        self.mode
    }

    pub(crate) fn with_field_statistics_coordinates() -> Self {
        Self::with_registry(SourceRegistry::with_field_statistics_coordinates)
    }

    fn with_registry(make: impl FnOnce(Arc<()>) -> SourceRegistry) -> Self {
        let identity = Arc::new(());
        let registry = Arc::new(Mutex::new(make(Arc::clone(&identity))));
        Self {
            registry,
            identity,
            mode: ProvenanceMode::Full,
        }
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
        if !registry.owns_use(actual) {
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

    /// Binds a value directly to one observed location with the same evidence as a separate use.
    /// A contributing scope from another inventory makes the source set incomplete.
    ///
    /// # Errors
    /// Returns an error if the observed scope's inventory is sealed or interrupted.
    pub fn value_at<T>(
        &self,
        scope: &SourceScope,
        location: SourceLocation,
        role: SourceUseRole,
        value: SourceValue<T>,
        source_set_complete: bool,
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        if self.mode == ProvenanceMode::Full || !Arc::ptr_eq(&self.registry, &scope.registry) {
            let used = scope.at(location, role)?;
            return Ok(self.value(value, &[used], source_set_complete));
        }
        let origin = source_evidence_error::lock(&scope.registry)?.origin_at(
            &scope.source,
            scope.scope,
            location,
            role,
            scope.encryption(),
            source_set_complete,
        );
        Ok(SourcedValue::new(value, origin))
    }

    /// Binds navigation to its actual contributing inputs without duplicating a game value.
    #[must_use]
    pub fn origin(&self, uses: &[SourceUse], source_set_complete: bool) -> FieldOrigin {
        let (own, complete) = self.owned_uses(uses, source_set_complete);
        FieldOrigin::new(Arc::clone(&self.identity), own, complete, self.mode)
    }

    /// Captures a contributor set shared by values derived from the same source survey.
    #[must_use]
    pub fn shared_origin(
        &self,
        uses: &[SourceUse],
        source_set_complete: bool,
    ) -> SharedFieldOrigin {
        let (own, complete) = self.owned_uses(uses, source_set_complete);
        SharedFieldOrigin::new(Arc::clone(&self.identity), own, complete, self.mode)
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
#[path = "provenance_mode_tests.rs"]
mod provenance_mode_tests;

#[cfg(test)]
#[path = "source_inventory_binding_tests.rs"]
mod binding_tests;

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
