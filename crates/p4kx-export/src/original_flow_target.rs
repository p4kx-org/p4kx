// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceEvidenceError, Step, Value};

use super::OriginalFlowFields;
use crate::catalog_read::original::{
    OriginalReferenceObservation, OriginalValueEdge, OriginalValueFact,
};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn target_fact(
        &self,
        observed: &OriginalReferenceObservation<'_, 'scope, 'read, 'source>,
    ) -> Result<Option<&OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        if !std::ptr::eq(observed.caller().scope(), self.scope) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        let admitted = observed.admitted();
        if admitted.origin.record_index() != self.record_index() {
            return Ok(None);
        }
        let (Some(positions), Some(location)) = (
            admitted.origin.instance_positions(),
            admitted.origin.instance_location(),
        ) else {
            return Ok(None);
        };
        if positions.len() != location.steps().len() || positions.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut at: Option<&OriginalValueFact<'_, '_, '_>> = None;
        for (position, step) in positions.iter().zip(location.steps()) {
            if at.is_some_and(|fact| matches!(fact.value(), Value::Error(_) | Value::Null)) {
                return Ok(None);
            }
            let edge = match step {
                Step::Field(_) => OriginalValueEdge::Field { ordinal: *position },
                Step::Element(element) if u64::from(*element) == *position => {
                    OriginalValueEdge::Element { ordinal: *position }
                }
                Step::Element(_) => {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            };
            let fact = self.child(at.map(OriginalValueFact::ordinal), edge)?;
            if let Step::Field(name) = step
                && fact.field().name() != *name
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            at = Some(fact);
        }
        match at {
            Some(fact)
                if matches!(fact.value(), Value::Struct(target)
                if target.type_name() == admitted.reference.type_name()) =>
            {
                Ok(Some(fact))
            }
            Some(fact) if matches!(fact.value(), Value::Error(_) | Value::Null) => Ok(None),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}
