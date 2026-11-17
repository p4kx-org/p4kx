// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::field_origin::OriginAccumulator;
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::{FieldOrigin, SourceValue, SourcedValue};

use super::SourceUses;

impl SourceUses {
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
