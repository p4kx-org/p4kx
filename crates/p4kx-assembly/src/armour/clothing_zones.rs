// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Guid;
use p4kx_extraction::{DataForgeOrigin, SourceLocation};

use super::{ArmourPart, BodyVariant, ClothingZoneCoverage, Wearable, ZoneChunk, ZoneList};

/// Authored body-zone evidence of the Entity that supplied a worn part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClothingZones {
    guid: Guid,
    record_index: Option<u32>,
    record_path: String,
    zones: ZoneList,
}

impl ClothingZones {
    pub(super) fn from_wearable(wearable: &Wearable) -> Self {
        Self {
            guid: wearable.guid,
            record_index: wearable.record_index,
            record_path: wearable.record_path.clone(),
            zones: wearable.zones.clone(),
        }
    }

    /// Returns the GUID of the Entity that supplied the part.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the physical Record index when it was retained.
    #[must_use]
    pub const fn record_index(&self) -> Option<u32> {
        self.record_index
    }

    /// Returns the Record path supplied with the Entity.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the decoded chunks in their authored order.
    #[must_use]
    pub fn chunks(&self) -> &[ZoneChunk] {
        &self.zones.chunks
    }

    /// Returns whether the complete clothing chunk list was readable from a physical Record.
    #[must_use]
    pub const fn chunks_complete(&self) -> bool {
        self.record_index.is_some() && !self.zones.truncated && !self.zones.incomplete
    }

    /// Iterates the original readable clothing chunks used to classify this zone.
    pub fn coverage_sources<'a>(
        &'a self,
        zone: &'a str,
    ) -> impl Iterator<Item = SourceLocation> + 'a {
        self.zones
            .known_chunks()
            .filter(move |chunk| chunk.zone() == zone)
            .filter_map(move |chunk| {
                let record_index = self.record_index?;
                let (component, row) = chunk.position?;
                Some(SourceLocation::DataForge {
                    record_index,
                    field: format!("Components[{component}].Chunks[{row}]"),
                })
            })
    }

    /// Returns the observed DataForge input when the resolver retained its binding.
    #[must_use]
    pub fn forge_origin(&self) -> Option<&DataForgeOrigin> {
        self.zones.forge_origin.as_deref()
    }

    pub(super) fn known_chunks(&self) -> impl Iterator<Item = &ZoneChunk> + Clone {
        self.zones.known_chunks()
    }

    /// Returns coverage inferred from readable clothing chunks for the supplied body variant.
    #[must_use]
    pub fn coverage(&self, variant: BodyVariant) -> ClothingZoneCoverage<'_> {
        ClothingZoneCoverage::of(self.zones.known_chunks(), variant)
    }
}

impl ArmourPart {
    /// Returns clothing-zone evidence when the part came from an Entity.
    #[must_use]
    pub fn clothing_zones(&self) -> Option<&ClothingZones> {
        self.clothing_zones.as_deref()
    }
}
