// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::field_origin::FieldOrigin;
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceInventory;
use crate::source_registry::SourceRegistry;
use crate::source_scope::{SourceInput, SourceScope};
use crate::source_use::{InputScope, SourceUse};
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
        let source = source_evidence_error::lock(&self.registry)?.observe(input);
        Ok(SourceScope {
            registry: Arc::clone(&self.registry),
            source: source.id,
            payload: source.payload,
            metadata: source.metadata,
            scope,
        })
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
            uses: registry.uses.values().cloned().collect(),
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
