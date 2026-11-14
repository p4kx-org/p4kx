// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The installation declaration retained from the Archive's directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationStatus {
    /// This layout carries no supported installation-status declaration.
    Unavailable,
    /// The known incomplete bit is clear; unknown bits retain their raw spelling.
    DeclaredComplete {
        /// The unmodified source status byte.
        flags: u8,
    },
}

impl InstallationStatus {
    /// Returns the source status byte, or none when no declaration is available.
    #[must_use]
    pub const fn raw_flags(self) -> Option<u8> {
        match self {
            Self::Unavailable => None,
            Self::DeclaredComplete { flags } => Some(flags),
        }
    }
}
