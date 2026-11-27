// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use std::collections::BTreeSet;

#[cfg(test)]
use p4kx_extraction::Guid;
use p4kx_extraction::{DataForge, Record};

use crate::Kind;
#[cfg(test)]
use crate::entity_selection::{Selected, select};
use crate::starmap_containers::Placements;

/// Declares the Record inventories covered by a scan and supplies placement inputs.
pub(crate) struct ScanDomain<'placement> {
    pub(super) placements: &'placement Placements,
    #[cfg(test)]
    quality_locations: Option<&'placement BTreeSet<Guid>>,
}

impl<'placement> ScanDomain<'placement> {
    /// Covers every ordinary Record inventory.
    pub(crate) const fn all(placements: &'placement Placements) -> Self {
        Self {
            placements,
            #[cfg(test)]
            quality_locations: None,
        }
    }

    /// Covers `Commodities`, `ResourceTypes` and `ResourceGroups`, including unreadable candidates.
    #[cfg(test)]
    pub(crate) const fn resources(
        placements: &'placement Placements,
        quality_locations: &'placement BTreeSet<Guid>,
    ) -> Self {
        Self {
            placements,
            quality_locations: Some(quality_locations),
        }
    }

    /// Keeps every candidate whose unreadable metadata could hide a resource Record.
    pub(super) fn records<'source>(
        &self,
        forge: &'source DataForge<'source>,
    ) -> Vec<Record<'source>> {
        let records = forge.records();
        #[cfg(test)]
        if let Some(locations) = self.quality_locations {
            return records
                .filter(|record| match select(record, None) {
                    Ok(None) => false,
                    Ok(Some(Selected::Entity(kind) | Selected::Leftover(kind))) => {
                        self.includes(kind)
                            || (kind == Kind::StarmapObjects && locations.contains(&record.guid()))
                    }
                    Err(_) => true,
                })
                .collect();
        }
        let _ = self;
        records.collect()
    }

    pub(super) const fn includes(&self, kind: Kind) -> bool {
        #[cfg(not(test))]
        let _ = (self, kind);
        #[cfg(test)]
        if self.quality_locations.is_some() {
            return matches!(
                kind,
                Kind::Commodities | Kind::ResourceTypes | Kind::ResourceGroups
            );
        }
        true
    }
}
