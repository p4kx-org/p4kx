// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Exact discriminator of the media contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MediaFormat {
    /// The media export envelope.
    #[serde(rename = "p4kx-media")]
    P4kxMedia,
}

/// Exact media format version, independent of catalog schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum MediaVersion {
    /// Version 1.0 of the media index and record envelopes.
    #[serde(rename = "1.0")]
    V1,
}

/// Returns the JSON Schema of the media index envelope.
#[must_use]
pub fn index_schema() -> Value {
    schemars::schema_for!(crate::Index).to_value()
}

/// Returns the JSON Schema of one media metadata record.
#[must_use]
pub fn record_schema() -> Value {
    schemars::schema_for!(crate::MediaRecord).to_value()
}

/// Returns the JSON Schema of the common typed reference envelope.
#[must_use]
pub fn reference_schema() -> Value {
    schemars::schema_for!(crate::Reference).to_value()
}

/// Returns the JSON Schema of one retained source inventory line.
#[must_use]
pub fn source_schema() -> Value {
    schemars::schema_for!(crate::Source).to_value()
}
