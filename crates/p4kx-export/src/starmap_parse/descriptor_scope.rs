// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Read policies for each Starmap descriptor consumer.

/// The placement details needed by the consumer of the descriptor walk.
#[derive(Clone, Copy)]
pub(crate) enum DescriptorScope {
    Catalog { jump_endpoints: bool },
    ContainerPlacements,
    LocationPlacements,
    ShopNames,
    Harvest,
    OpenedContainers,
    LootPlacements,
}

impl DescriptorScope {
    pub(super) const fn captures_loot_survey(self) -> bool {
        matches!(self, Self::LootPlacements)
    }

    pub(super) const fn reads_location_declarations(self) -> bool {
        matches!(
            self,
            Self::Catalog { .. } | Self::LocationPlacements | Self::ShopNames
        )
    }

    pub(super) const fn reads_location_companions(self) -> bool {
        matches!(self, Self::Catalog { .. } | Self::LocationPlacements)
    }

    pub(super) const fn resolves_placements(self) -> bool {
        !matches!(self, Self::Harvest)
    }

    pub(super) const fn reads_location_physics(self) -> bool {
        matches!(self, Self::Catalog { .. } | Self::ContainerPlacements)
    }

    pub(super) const fn reads_jump_endpoints(self) -> bool {
        matches!(
            self,
            Self::Catalog {
                jump_endpoints: true
            }
        )
    }
}
