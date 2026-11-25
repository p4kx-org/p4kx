// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Selects how much of the original contribution evidence a result exposes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum ProvenanceMode {
    /// Retains each field's exact source-use references and original evidence.
    Full,
    /// Retains each field's immediate encryption marker without its source graph.
    #[default]
    MarkersOnly,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EncryptionMarker {
    pub(crate) contains_encrypted_source: Option<bool>,
}
