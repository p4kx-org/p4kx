// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::{FieldOrigin, SourceUse, SourceValue, SourcedValue};

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
        if origins
            .iter()
            .any(|origin| !origin.belongs_to(&self.identity))
        {
            return Err(SourceEvidenceError::ForeignEvidence);
        }
        let registry = source_evidence_error::lock(&self.registry)?;
        let complete = origins.iter().all(|origin| origin.source_set_complete());
        let uses = origins
            .iter()
            .flat_map(|origin| origin.source_uses_refs())
            .map(|reference| {
                registry
                    .uses
                    .get(reference)
                    .map(|description| SourceUse {
                        description: Arc::clone(description),
                        identity: Arc::clone(&self.identity),
                    })
                    .ok_or(SourceEvidenceError::ForeignEvidence)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.value(value, &uses, complete))
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
