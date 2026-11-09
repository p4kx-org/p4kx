// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::field_origin::FieldOrigin;
use crate::source_details::{SourceCoordinateOmission, SourceDetails};
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceInventory;
use crate::source_registry::SourceRegistry;
use crate::source_scope::{SourceInput, SourceScope};
use crate::source_use::{InputScope, SourceLocation, SourceUse};
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
        let identity = Arc::new(());
        Self {
            registry: Arc::new(Mutex::new(SourceRegistry::new(Arc::clone(&identity)))),
            identity,
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
        let foreign = uses.iter().any(|source| {
            source.role().contributes() && !Arc::ptr_eq(&source.identity, &self.identity)
        });
        let own = uses
            .iter()
            .filter(|source| Arc::ptr_eq(&source.identity, &self.identity));
        SourcedValue::new(
            value,
            FieldOrigin::new(own, source_set_complete && !foreign),
        )
    }

    /// Freezes sorted source/use rows after projection for repeatable serialization passes.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn freeze(&self) -> Result<SourceInventory, SourceEvidenceError> {
        let registry = source_evidence_error::lock(&self.registry)?;
        Ok(SourceInventory {
            sources: registry.sources.values().cloned().collect(),
            uses: registry
                .uses
                .values()
                .map(|description| description.as_ref().clone())
                .collect(),
        })
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
