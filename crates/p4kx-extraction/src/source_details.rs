// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// Detail retention independently of original input identity and encryption proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceDetails {
    /// Retain complete stored paths and declaration details.
    Full,
    /// Retain original ownership and encryption proof without variable detail copies.
    CaptureLimited,
}

/// Why an actual contributing use omits its optional source coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceCoordinateOmission {
    /// Optional coordinate capture was not admitted.
    CaptureLimit,
    /// A proved original contribution has no retained source coordinate.
    Unavailable,
}

impl SourceCoordinateOmission {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::CaptureLimit => "captureLimit",
            Self::Unavailable => "unavailable",
        }
    }
}

#[cfg(test)]
#[path = "source_coordinate_omission_tests.rs"]
mod coordinate_tests;
