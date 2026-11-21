// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// How a contract ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum EndReason {
    /// The player completed it (`Complete`).
    Complete,
    /// It failed (`Fail`).
    Fail,
    /// The player abandoned it (`Abandon`).
    Abandon,
}

impl EndReason {
    /// Returns the reason the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Complete" => Self::Complete,
            "Fail" => Self::Fail,
            "Abandon" => Self::Abandon,
            _ => return None,
        })
    }
}
