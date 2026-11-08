// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::{Digest, Source, Subresource};

/// Why required input bytes have no published raw reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum InputOmission {
    /// The individual required texture part is explicitly excluded from output.
    ExplicitExclusion,
    /// Private read used only to establish an exported reference target.
    ReferenceLookup,
}

/// Verified physical input evidence, independent of callback selection.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[schemars(extend("required" = ["source", "digest", "omission", "use_at"]))]
#[non_exhaustive]
pub struct InputReceipt {
    /// Physical source with a raw reference only when its bytes are published.
    pub source: Source,
    /// Measured complete materialised-payload digest, independent of reader-use ranges.
    pub digest: Digest,
    /// Explicit omission reason, or null for a retained payload.
    pub omission: Option<InputOmission>,
    /// Source-local declaration that actually requires this input.
    pub use_at: Subresource,
}
