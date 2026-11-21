// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::PaintTagIndex;

use crate::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::paint_port::{PaintPort, VehicleFacts};

#[cfg(test)]
#[path = "paint_matching_tests.rs"]
mod tests;

/// Identity of a physical participant in one paint compatibility decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PaintOwner {
    pub(super) record_index: u32,
    pub(super) guid: CatalogGuid,
}

/// Ships grouped by their effective ports and tags.
pub(super) struct ShipPaintMatches {
    groups: HashMap<ShipPorts, Vec<PaintOwner>>,
}

impl ShipPaintMatches {
    pub(super) fn read(vehicles: &[VehicleFacts]) -> Result<Self, ExportAccountingError> {
        let mut groups = HashMap::<ShipPorts, Vec<PaintOwner>>::new();
        for vehicle in vehicles {
            let record_index = vehicle
                .source_record_index
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            let mut ports = vehicle.paint_ports.clone();
            ports.extend(vehicle.implementation.ports.iter().cloned());
            if ports.is_empty() {
                continue;
            }
            ports.sort_unstable();
            ports.dedup();
            let mut tags = vehicle.tags.clone();
            tags.sort_unstable();
            tags.dedup();
            groups
                .entry(ShipPorts { ports, tags })
                .or_default()
                .push(PaintOwner {
                    record_index,
                    guid: vehicle.ship,
                });
        }
        Ok(Self { groups })
    }

    /// Iterates the physical ships whose effective ports accept the paint.
    pub(super) fn fit<'scope>(
        &'scope self,
        tags: &[&str],
        required_tags: &[&str],
        index: &'scope PaintTagIndex<'_>,
    ) -> impl Iterator<Item = PaintOwner> + 'scope {
        let have = lower_tags(tags);
        let required = lower_tags(required_tags);
        self.groups
            .iter()
            .filter(move |(group, _)| group.accepts(&have, &required, index))
            .flat_map(|(_, ships)| ships.iter().copied())
    }
}

/// The port and tag signature used to share compatibility work across ships.
#[derive(Debug, PartialEq, Eq, Hash)]
struct ShipPorts {
    ports: Vec<PaintPort>,
    tags: Vec<String>,
}

impl ShipPorts {
    fn accepts(&self, have: &[String], required: &[String], index: &PaintTagIndex<'_>) -> bool {
        self.ports
            .iter()
            .any(|port| port.accepts(have, required, &self.tags, index))
    }
}

fn lower_tags(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| tag.to_ascii_lowercase()).collect()
}
