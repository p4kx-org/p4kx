// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;
use p4kx_extraction::cryxml::SourceAttribute;
use p4kx_model::{SocAreaLink, SocRoomDeclaration, SocRoomRegistry};

/// An index over the valid GUID declarations of one physical container registry.
#[derive(Debug)]
pub struct SocRoomIndex<'document> {
    by_guid: BTreeMap<Guid, Vec<SocRoomDeclaration<'document>>>,
}

impl<'document> SocRoomIndex<'document> {
    /// Indexes original room declarations within one registry.
    #[must_use]
    pub fn new(registry: &'document SocRoomRegistry<'_>) -> Self {
        let mut by_guid: BTreeMap<Guid, Vec<_>> = BTreeMap::new();
        for room in registry.rooms() {
            if let SocRoomGuid::Present(guid) = SocRoomGuid::read(room.guid_attributes()) {
                by_guid.entry(guid).or_default().push(room);
            }
        }
        Self { by_guid }
    }

    /// Resolves an authored target only within this registry's physical scope.
    #[must_use]
    pub fn target(&self, link: SocAreaLink<'_>) -> SocRoomTarget<'_, 'document> {
        let guid = match SocRoomGuid::read(link.guid_attributes()) {
            SocRoomGuid::Absent => return SocRoomTarget::Absent,
            SocRoomGuid::Invalid => return SocRoomTarget::Invalid,
            SocRoomGuid::Present(guid) => guid,
        };
        match self.by_guid.get(&guid).map(Vec::as_slice) {
            None => SocRoomTarget::Unmatched { guid },
            Some([room]) => SocRoomTarget::Matched { guid, room: *room },
            Some(rooms) => SocRoomTarget::Ambiguous { guid, rooms },
        }
    }
}

/// The stored GUID of an original room declaration or target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocRoomGuid {
    /// No unqualified GUID attribute is stored.
    Absent,
    /// The original attribute is malformed or repeated.
    Invalid,
    /// One well-formed GUID is stored.
    Present(Guid),
}

impl SocRoomGuid {
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

/// A source target's state within a successfully read room registry.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SocRoomTarget<'index, 'document> {
    /// No target attribute was authored.
    Absent,
    /// The target attribute is malformed or repeated.
    Invalid,
    /// The GUID is absent from this registry.
    Unmatched {
        /// The exact original identity.
        guid: Guid,
    },
    /// One physical declaration resolves the original GUID.
    Matched {
        /// The exact original identity.
        guid: Guid,
        /// The one declaration belonging to this registry.
        room: SocRoomDeclaration<'document>,
    },
    /// More than one physical declaration owns the original GUID.
    Ambiguous {
        /// The exact original identity.
        guid: Guid,
        /// All candidate declarations in original registry order.
        rooms: &'index [SocRoomDeclaration<'document>],
    },
}
