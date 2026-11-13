// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;
use p4kx_extraction::cryxml::SourceAttribute;

/// The GUID of an original SOC declaration or target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocSourceGuid {
    /// No unqualified GUID attribute is stored.
    Absent,
    /// The original attribute is malformed or repeated.
    Invalid,
    /// One well-formed GUID is stored.
    Present(Guid),
}

impl SocSourceGuid {
    /// Reads the exact GUID attributes without selecting one from duplicates.
    pub fn read<'document>(
        mut attributes: impl Iterator<Item = SourceAttribute<'document>>,
    ) -> Self {
        let Some(attribute) = attributes.next() else {
            return Self::Absent;
        };
        if attributes.next().is_some() {
            return Self::Invalid;
        }
        attribute
            .value()
            .parse()
            .map_or(Self::Invalid, Self::Present)
    }
}
