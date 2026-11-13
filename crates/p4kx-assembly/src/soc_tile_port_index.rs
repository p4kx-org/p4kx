// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::cryxml::SourceElementPosition;
use p4kx_model::{SocContainerDescriptor, SocSourceValue, SocTileAttachment, SocTileItemPort};

/// Tile item ports indexed within one physical container descriptor.
#[derive(Debug, Default)]
pub struct SocTilePortIndex<'document> {
    by_name: BTreeMap<&'document str, Vec<SocTilePortSite<'document>>>,
}

impl<'document> SocTilePortIndex<'document> {
    /// Indexes the original port declarations with uniquely authored names.
    #[must_use]
    pub fn read(descriptor: &'document SocContainerDescriptor<'_>) -> Self {
        let mut index = Self::default();
        for port in descriptor.tile_item_ports() {
            let SocSourceValue::Present(name) = port.name() else {
                continue;
            };
            index
                .by_name
                .entry(name)
                .or_default()
                .push(SocTilePortSite {
                    position: port.element().position(),
                    port,
                    offset: port.interaction_offset(),
                });
        }
        index
    }

    /// Resolves a tile selector against the original ports of this descriptor.
    #[must_use]
    pub fn resolve<'index>(
        &'index self,
        attachment: SocTileAttachment<'_>,
    ) -> SocTilePortTarget<'index, 'document> {
        let name = match attachment.item_port() {
            SocSourceValue::Present(name) => name,
            SocSourceValue::Absent => return SocTilePortTarget::Absent,
            _ => return SocTilePortTarget::Invalid,
        };
        let Some(sites) = self.by_name.get(name) else {
            return SocTilePortTarget::Unmatched;
        };
        match sites.as_slice() {
            [site] => SocTilePortTarget::Matched(site),
            many => SocTilePortTarget::Ambiguous(many),
        }
    }
}

/// The original port coordinate and independently read offset.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SocTilePortSite<'document> {
    /// Original descriptor XML coordinate.
    pub position: SourceElementPosition,
    /// Original port element and attribute positions.
    pub port: SocTileItemPort<'document>,
    /// Original interaction offset in the container owner's frame.
    pub offset: SocSourceValue<[f64; 3]>,
}

/// The outcome of resolving an original tile attachment selector.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SocTilePortTarget<'index, 'document> {
    /// The attachment does not declare a port name.
    Absent,
    /// The attachment declares an empty, malformed or repeated port name.
    Invalid,
    /// The original selector has no port in this descriptor.
    Unmatched,
    /// One original port matches the selector.
    Matched(&'index SocTilePortSite<'document>),
    /// Several original ports match the selector.
    Ambiguous(&'index [SocTilePortSite<'document>]),
}
