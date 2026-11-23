// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::field_origin::OriginAccumulator;
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::{
    FieldOrigin, SharedFieldOrigin, SourceUse, SourceUseRef, SourceUseRole, SourceValue,
    SourcedValue,
};

use super::SourceUses;

impl SourceUses {
    /// Verifies that references identify attempted observations in this capture.
    ///
    /// # Errors
    /// Returns an error for a foreign reference, unexpected role or unavailable registry.
    pub fn verify_attempted_uses(
        &self,
        references: &[SourceUseRef],
    ) -> Result<(), SourceEvidenceError> {
        if references.is_empty() {
            return Ok(());
        }
        let registry = source_evidence_error::lock(&self.registry)?;
        for reference in references {
            let registered = registry
                .uses
                .contribution(reference)
                .ok_or(SourceEvidenceError::ForeignEvidence)?;
            let original = registered.id.retain();
            let same_observation = original
                .registered_text()
                .zip(reference.registered_text())
                .is_some_and(|(original, supplied)| Arc::ptr_eq(original, supplied));
            if !same_observation {
                return Err(SourceEvidenceError::ForeignEvidence);
            }
            if registered.role != SourceUseRole::Attempted {
                return Err(SourceEvidenceError::UnexpectedUseRole);
            }
        }
        drop(registry);
        Ok(())
    }

    pub(crate) fn shared_origin_from_uses<'source>(
        &self,
        uses: impl IntoIterator<Item = &'source SourceUse>,
        complete: bool,
    ) -> Result<SharedFieldOrigin, SourceEvidenceError> {
        Ok(self
            .measure_uses(uses, complete)?
            .finish_shared(Arc::clone(&self.identity)))
    }

    pub(crate) fn origin_with_shared_uses<'source>(
        &self,
        shared: &SharedFieldOrigin,
        uses: impl IntoIterator<Item = &'source SourceUse>,
        complete: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        if !shared.belongs_to(&self.identity) {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        Ok(self
            .measure_uses(uses, complete)?
            .finish_with_shared(shared))
    }

    fn measure_uses<'source>(
        &self,
        uses: impl IntoIterator<Item = &'source SourceUse>,
        complete: bool,
    ) -> Result<OriginAccumulator, SourceEvidenceError> {
        let mut origin = OriginAccumulator::new(complete);
        for incoming in uses {
            if !Arc::ptr_eq(&incoming.identity, &self.identity) {
                return Err(SourceEvidenceError::ForeignEvidence);
            }
            origin.observe(incoming.description().into());
        }
        Ok(origin)
    }

    /// Combines observed origins through this capture's physical source registry.
    ///
    /// # Errors
    /// Returns an error when referenced physical observations are unavailable.
    pub fn value_with_origins<T>(
        &self,
        value: SourceValue<T>,
        origins: &[&FieldOrigin],
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let origin = self.origin_with_origins(origins.iter().copied(), true)?;
        Ok(SourcedValue::new(value, origin))
    }

    /// Adds already captured contributions to a value while retaining its state and payload.
    ///
    /// # Errors
    /// Returns an error when any origin is foreign or its original observations are unavailable.
    pub fn add_value_origins<T>(
        &self,
        value: SourcedValue<T>,
        origins: &[&FieldOrigin],
    ) -> Result<SourcedValue<T>, SourceEvidenceError> {
        let origins = std::iter::once(value.origin()).chain(origins.iter().copied());
        let origin = self.origin_with_origins(origins, true)?;
        Ok(value.with_origin(origin))
    }

    /// Binds an aggregate to its already registered contributors and local completeness.
    ///
    /// # Errors
    /// Returns an error when a contributor is foreign or cannot be resolved.
    pub fn origin_with_origins<'a>(
        &self,
        origins: impl IntoIterator<Item = &'a FieldOrigin>,
        complete: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let registry = source_evidence_error::lock(&self.registry)?;
        let mut accumulator = OriginAccumulator::new(complete);
        for origin in origins {
            if !origin.belongs_to(&self.identity) {
                return Err(SourceEvidenceError::ForeignEvidence);
            }
            accumulator.include_completeness(origin.source_set_complete());
            for reference in origin.iter_source_use_refs() {
                let contribution = registry
                    .uses
                    .contribution(reference)
                    .ok_or(SourceEvidenceError::ForeignEvidence)?;
                accumulator.observe(contribution);
            }
        }
        drop(registry);
        Ok(accumulator.finish(Arc::clone(&self.identity)))
    }
}

#[cfg(test)]
mod tests {
    use crate::{InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUseRole};

    use super::SourceUses;

    #[test]
    fn original_use_union_rejects_a_foreign_physical_input() -> Result<(), SourceEvidenceError> {
        let owner = SourceUses::with_compact_coordinates();
        let other = SourceUses::with_compact_coordinates();
        let source = other
            .observe(SourceInput::Unknown, InputScope::Payload)?
            .at(
                SourceLocation::DataForge {
                    record_index: 0,
                    field: "value".into(),
                },
                SourceUseRole::Selector,
            )?;
        assert!(
            matches!(
                owner.shared_origin_from_uses([&source], true),
                Err(SourceEvidenceError::ForeignEvidence)
            ),
            "a compact coordinate cannot create original source identity in a different capture"
        );
        let empty_basis = owner.shared_origin_from_uses([], true)?;
        assert_eq!(
            owner.origin_with_shared_uses(&empty_basis, [&source], true),
            Err(SourceEvidenceError::ForeignEvidence),
            "a foreign local operand cannot be added to the summary"
        );
        assert_eq!(
            other.origin_with_shared_uses(&empty_basis, [], true),
            Err(SourceEvidenceError::ForeignEvidence),
            "a shared set cannot cross a physical source inventory"
        );
        Ok(())
    }

    #[test]
    fn attempted_refs_require_original_inventory_and_attempt_role()
    -> Result<(), SourceEvidenceError> {
        let local = SourceUses::with_compact_use_refs();
        let other = SourceUses::with_compact_use_refs();
        let observe = |sources: &SourceUses, role| {
            sources
                .observe(SourceInput::Unknown, InputScope::Payload)?
                .at(
                    SourceLocation::DataForge {
                        record_index: 0,
                        field: String::new(),
                    },
                    role,
                )
        };
        let local_attempt = observe(&local, SourceUseRole::Attempted)?;
        let foreign_attempt = observe(&other, SourceUseRole::Attempted)?;
        let local_payload = observe(&local, SourceUseRole::Payload)?;
        assert_eq!(
            local_attempt.reference(),
            foreign_attempt.reference(),
            "identical compact references can be minted by separate captures"
        );
        local.verify_attempted_uses(std::slice::from_ref(local_attempt.reference()))?;
        assert_eq!(
            local.verify_attempted_uses(std::slice::from_ref(foreign_attempt.reference())),
            Err(SourceEvidenceError::ForeignEvidence),
            "the same wire string does not prove actual capture identity"
        );
        assert_eq!(
            local.verify_attempted_uses(std::slice::from_ref(local_payload.reference())),
            Err(SourceEvidenceError::UnexpectedUseRole),
            "a successful payload cannot be relabeled as an attempted read"
        );
        Ok(())
    }

    #[test]
    fn combined_origins_require_original_inventory_identity() -> Result<(), SourceEvidenceError> {
        let local = SourceUses::with_compact_use_refs();
        let other = SourceUses::with_compact_use_refs();
        let observe = |sources: &SourceUses, field: &str| {
            sources
                .observe(SourceInput::Unknown, InputScope::Payload)?
                .at(
                    SourceLocation::Metadata {
                        field: field.into(),
                    },
                    SourceUseRole::Payload,
                )
        };
        let first = observe(&local, "a")?;
        let second = observe(&local, "b")?;
        let foreign = observe(&other, "a")?;
        assert_eq!(
            first.reference(),
            foreign.reference(),
            "the documents reuse a short reference"
        );
        let left = local.origin(std::slice::from_ref(&first), true);
        let right = local.origin(std::slice::from_ref(&second), false);
        let combined =
            local.value_with_origins(crate::SourceValue::Present(5), &[&left, &right, &left])?;
        assert_eq!(
            combined.origin(),
            &local.origin(&[first, second], false),
            "combined origin has exactly the observed uses and completeness"
        );
        let foreign = other.origin(&[foreign], true);
        assert!(
            matches!(
                local.value_with_origins(crate::SourceValue::Present(5), &[&foreign]),
                Err(SourceEvidenceError::ForeignEvidence)
            ),
            "equal wire references cannot restore authority for a foreign value"
        );
        Ok(())
    }
}
