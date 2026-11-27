// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::Schema;

/// Requires every property of an always-present object projection.
pub(crate) fn require_properties(schema: &mut Schema) {
    if let Some(properties) = schema.get("properties")
        && let Some(properties) = properties.as_object()
    {
        let required = properties.keys().cloned().collect::<Vec<_>>();
        schema.insert("required".into(), required.into());
    } else {
        *schema = Schema::from(false);
    }
}
