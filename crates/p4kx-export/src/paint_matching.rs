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
pub(crate) struct PaintOwner {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
}

/// Ships grouped by their effective ports and tags.
#[derive(Default)]
pub(super) struct ShipPaintMatches {
    groups: HashMap<ShipPorts, Vec<PaintOwner>>,
}

impl ShipPaintMatches {
    pub(super) fn read(vehicles: &[VehicleFacts]) -> Result<Self, ExportAccountingError> {
        let mut matches = Self::default();
        for vehicle in vehicles {
            let record_index = vehicle
                .source_record_index
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            let mut ports = vehicle.paint_ports.clone();
            ports.extend(vehicle.implementation.ports.iter().cloned());
            matches.add(
                PaintOwner {
                    record_index,
                    guid: vehicle.ship,
                },
                ports,
                || vehicle.tags.clone(),
            );
        }
        Ok(matches)
    }

    /// Retains one physical ship after folding its effective Record and XML ports.
    pub(super) fn add(
        &mut self,
        owner: PaintOwner,
        mut ports: Vec<PaintPort>,
        tags: impl FnOnce() -> Vec<String>,
    ) {
        if ports.is_empty() {
            return;
        }
        ports.sort_unstable();
        ports.dedup();
        let mut tags = tags();
        tags.sort_unstable();
        tags.dedup();
        self.groups
            .entry(ShipPorts { ports, tags })
            .or_default()
            .push(owner);
    }

    /// Iterates the physical ships whose effective ports accept the paint.
    pub(super) fn fit<'scope>(
        &'scope self,
        tags: &[&str],
        required_tags: &[&str],
        index: &'scope PaintTagIndex<'_>,
    ) -> impl Iterator<Item = PaintOwner> + 'scope {
        let paint = PaintRequirements::new(tags, required_tags);
        self.groups
            .iter()
            .filter(move |(group, _)| paint.accepts(&group.ports, &group.tags, index))
            .flat_map(|(_, ships)| ships.iter().copied())
    }
}

/// The port and tag signature used to share compatibility work across ships.
#[derive(Debug, PartialEq, Eq, Hash)]
struct ShipPorts {
    ports: Vec<PaintPort>,
    tags: Vec<String>,
}

pub(crate) struct PaintRequirements {
    offered: Vec<String>,
    required: Vec<String>,
}

impl PaintRequirements {
    pub(crate) fn new(tags: &[&str], required_tags: &[&str]) -> Self {
        Self {
            offered: lower_tags(tags),
            required: lower_tags(required_tags),
        }
    }

    pub(crate) fn accepts(
        &self,
        ports: &[PaintPort],
        ship_tags: &[String],
        index: &PaintTagIndex<'_>,
    ) -> bool {
        ports
            .iter()
            .any(|port| port.accepts(&self.offered, &self.required, ship_tags, index))
    }
}

fn lower_tags(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| tag.to_ascii_lowercase()).collect()
}
