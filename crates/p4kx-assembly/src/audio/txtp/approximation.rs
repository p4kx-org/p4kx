// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A decision made while rendering a resolved event as TXTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TxtpApproximation {
    /// The rendered choice selects its first available alternative.
    SelectFirstAlternative,
}

impl TxtpApproximation {
    /// Returns the stable code used in reports and TXTP comments.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SelectFirstAlternative => "select-first-alternative",
        }
    }
}
