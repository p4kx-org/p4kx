// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Value, ValueError};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::export_field_error;

/// Whether all original values are readable or retain a complete unresolved Reference identity.
pub(crate) fn retained_values_complete(fields: &OriginalFlowFields<'_, '_, '_>) -> bool {
    complete(fields.facts(), |error| {
        !matches!(
            error,
            ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. }
        )
    })
}

/// Whether selected original facts pass the canonical raw source inspection.
pub(crate) fn retained_facts_complete(facts: &[OriginalValueFact<'_, '_, '_>]) -> bool {
    complete(facts, export_field_error::blocks_projection)
}

fn complete(
    facts: &[OriginalValueFact<'_, '_, '_>],
    blocks_projection: impl Fn(ValueError) -> bool,
) -> bool {
    facts
        .iter()
        .all(|fact| !matches!(fact.value(), Value::Error(error) if blocks_projection(*error)))
}
