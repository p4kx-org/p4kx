// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;
use p4kx_extraction::cryxml::SourceElementPosition;
use p4kx_model::{SocEntityChunk, SocEntityLink};

use crate::soc_source_guid::SocSourceGuid;

/// Authored SOC Entity owners from a single physical container.
#[derive(Debug, Default)]
pub struct SocEntityIndex {
    by_guid: BTreeMap<Guid, Vec<SocEntitySite>>,
}

impl SocEntityIndex {
    /// Creates an empty index for one physical container.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            by_guid: BTreeMap::new(),
        }
    }

    /// Adds the original Entity occurrences of one chunk in a container member.
    pub fn add_chunk(&mut self, stream_index: usize, chunk: &SocEntityChunk<'_>) {
        for entity in chunk.entities() {
            let SocSourceGuid::Present(guid) = SocSourceGuid::read(entity.guid_attributes()) else {
                continue;
            };
            self.by_guid.entry(guid).or_default().push(SocEntitySite {
                stream_index,
                chunk_index: chunk.chunk_index(),
                entity_ordinal: entity.ordinal(),
                position: entity.element().position(),
            });
        }
    }

    /// Resolves an authored link against the Entity declarations indexed for this container.
    #[must_use]
    pub fn target(&self, link: SocEntityLink<'_>) -> SocEntityTarget<'_> {
        let guid = match SocSourceGuid::read(link.target_attributes()) {
            SocSourceGuid::Absent => return SocEntityTarget::Absent,
            SocSourceGuid::Invalid => return SocEntityTarget::Invalid,
            SocSourceGuid::Present(guid) => guid,
        };
        match self.by_guid.get(&guid).map(Vec::as_slice) {
            None => SocEntityTarget::Unmatched { guid },
            Some([site]) => SocEntityTarget::Matched { guid, site },
            Some(sites) => SocEntityTarget::Ambiguous { guid, sites },
        }
    }
}

/// The original member, chunk, and Entity position of a link target.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SocEntitySite {
    /// Physical entry index of the SOC stream in its owning container.
    pub stream_index: usize,
    /// Physical index in the SOC stream's chunk table.
    pub chunk_index: usize,
    /// Original Entity slot within the chunk's direct elements.
    pub entity_ordinal: usize,
    /// Original XML coordinate within the Entity chunk.
    pub position: SourceElementPosition,
}

/// A link's state relative to the indexed physical container.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SocEntityTarget<'index> {
    /// No target attribute is stored.
    Absent,
    /// The stored target is malformed or repeated.
    Invalid,
    /// The GUID has no owner among the indexed chunks.
    Unmatched {
        /// The exact original identity.
        guid: Guid,
    },
    /// One physical Entity owns this GUID.
    Matched {
        /// The exact original identity.
        guid: Guid,
        /// The single physical declaration.
        site: &'index SocEntitySite,
    },
    /// More than one physical Entity owns this GUID.
    Ambiguous {
        /// The exact original identity.
        guid: Guid,
        /// The physical declarations in indexing order.
        sites: &'index [SocEntitySite],
    },
}
