// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

/// Why an attached item's authored offset helper could not be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ItemOffsetDiagnostic {
    /// The attached model has no node with the authored helper name.
    #[error("the attached model has no item offset helper {helper}. Using the port placement")]
    NotFound {
        /// The authored helper name.
        helper: String,
    },
    /// The model hierarchy does not place the named node.
    #[error(
        "item offset helper {helper} has no placement in the attached model. Using the port placement"
    )]
    InvalidPlacement {
        /// The authored helper name.
        helper: String,
    },
    /// The named node's placement cannot be inverted.
    #[error("item offset helper {helper} has no invertible transform. Using the port placement")]
    NotInvertible {
        /// The authored helper name.
        helper: String,
    },
}

impl ItemOffsetDiagnostic {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "item_offset_helper_not_found",
            Self::InvalidPlacement { .. } => "item_offset_helper_invalid_placement",
            Self::NotInvertible { .. } => "item_offset_helper_not_invertible",
        }
    }
}
