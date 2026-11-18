// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Value, ValueError};

use crate::catalog_read::original::OriginalFlowFields;

/// Whether all original values are readable or retain a complete unresolved Reference identity.
pub(crate) fn retained_values_complete(fields: &OriginalFlowFields<'_, '_, '_>) -> bool {
    fields.facts().iter().all(|fact| !matches!(fact.value(), Value::Error(error)
        if !matches!(error, ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. })))
}
