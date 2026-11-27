// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// Version of source-layout payloads independently of the media envelope.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[non_exhaustive]
pub enum ArchiveMetadataVersion {
    /// Source-layout payload version 1.0.
    #[serde(rename = "1.0")]
    V1,
}
