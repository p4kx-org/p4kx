// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// Whether the output has an Archive payload origin.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub enum OriginApplicability {
    /// This output contains only operational or installation metadata.
    NotApplicable,
}
