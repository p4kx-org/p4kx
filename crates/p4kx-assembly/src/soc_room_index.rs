// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::soc_source_guid::SocSourceGuid;
use p4kx_extraction::Guid;
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
            if let SocSourceGuid::Present(guid) = SocSourceGuid::read(room.guid_attributes()) {
                by_guid.entry(guid).or_default().push(room);
            }
        }
        Self { by_guid }
    }

    /// Resolves an authored target only within this registry's physical scope.
    #[must_use]
    pub fn target(&self, link: SocAreaLink<'_>) -> SocRoomTarget<'_, 'document> {
        let guid = match SocSourceGuid::read(link.guid_attributes()) {
            SocSourceGuid::Absent => return SocRoomTarget::Absent,
            SocSourceGuid::Invalid => return SocRoomTarget::Invalid,
            SocSourceGuid::Present(guid) => guid,
        };
        match self.by_guid.get(&guid).map(Vec::as_slice) {
            None => SocRoomTarget::Unmatched { guid },
            Some([room]) => SocRoomTarget::Matched { guid, room: *room },
            Some(rooms) => SocRoomTarget::Ambiguous { guid, rooms },
        }
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
