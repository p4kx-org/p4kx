// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::aggregate_evidence::Checkpoint;
use super::{
    VehicleAggregateEvidence, VehicleImplementation, VehicleItemPort, VehiclePart, VehicleXmlSite,
};
use crate::merge::port_key;

#[derive(Debug)]
struct Prefix<'vehicle> {
    evidence: &'vehicle VehicleAggregateEvidence,
    end: Checkpoint,
}

impl<'vehicle> Prefix<'vehicle> {
    fn sites(&self) -> impl Iterator<Item = &'vehicle VehicleXmlSite> {
        self.evidence.prefix_selectors(self.end)
    }

    fn is_complete(&self) -> bool {
        self.evidence.prefix_complete(self.end)
    }
}

/// The effective item port and retained source observations for its selection.
#[derive(Debug)]
pub struct VehiclePortSelection<'vehicle> {
    part: &'vehicle VehiclePart,
    port: &'vehicle VehicleItemPort,
    ordinal: usize,
    documents: Option<Prefix<'vehicle>>,
    parts: Option<Prefix<'vehicle>>,
}

impl<'vehicle> VehiclePortSelection<'vehicle> {
    /// Borrows the selected item port.
    #[must_use]
    pub const fn port(&self) -> &'vehicle VehicleItemPort {
        self.port
    }

    /// Borrows the physical Part that supplies this port.
    #[must_use]
    pub const fn part(&self) -> &'vehicle VehiclePart {
        self.part
    }

    /// Returns the port's position in the effective depth-first item port traversal.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Borrows the original effective `ItemPort` element site, when observed.
    #[must_use]
    pub fn position(&self) -> Option<&'vehicle VehicleXmlSite> {
        self.port.origins.element()
    }

    /// Iterates the consulted document and Part selectors.
    pub fn selectors(&self) -> impl Iterator<Item = &'vehicle VehicleXmlSite> {
        self.documents
            .iter()
            .chain(self.parts.iter())
            .flat_map(Prefix::sites)
    }

    /// Reports whether the observed XML selection retained all original coordinates.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.documents.as_ref().is_some_and(Prefix::is_complete)
            && self.parts.as_ref().is_some_and(Prefix::is_complete)
    }

    /// Reports whether an explicitly missing patch is the only gap in the original XML selection.
    /// The caller must separately prove the exact patch absence from its original file lookup.
    #[must_use]
    pub fn missing_explicit_patch_is_only_gap(&self) -> bool {
        self.documents.as_ref().is_some_and(|documents| {
            documents
                .evidence
                .prefix_has_only_missing_explicit_patch(documents.end)
        }) && self.parts.as_ref().is_some_and(Prefix::is_complete)
    }
}

impl VehicleImplementation {
    /// Returns the first item port of the given name with its optional original XML selectors.
    #[must_use]
    pub fn item_port_with_selection(&self, name: &str) -> Option<VehiclePortSelection<'_>> {
        let indexed = self.port_paths.get(&port_key(name))?;
        let part = self.part_at(&indexed.path)?;
        self.port_selection(part, indexed.ordinal)
    }

    /// Iterates every effective item port and its available original XML observations.
    /// Retains separate physical occurrences in depth-first order, including equal names.
    pub fn item_ports_with_selection(&self) -> impl Iterator<Item = VehiclePortSelection<'_>> {
        self.item_ports()
            .enumerate()
            .filter_map(move |(ordinal, part)| self.port_selection(part, ordinal))
    }

    fn port_selection<'vehicle>(
        &'vehicle self,
        part: &'vehicle VehiclePart,
        ordinal: usize,
    ) -> Option<VehiclePortSelection<'vehicle>> {
        let item_port = part.item_port()?;
        let origins = &self.declarations;
        let documents = origins
            .aggregate
            .as_ref()
            .zip(origins.port_documents)
            .map(|(evidence, end)| Prefix { evidence, end });
        let parts = origins
            .port_parts
            .as_ref()
            .zip(origins.port_occurrences.get(ordinal).copied())
            .map(|(evidence, end)| Prefix { evidence, end });
        Some(VehiclePortSelection {
            part,
            port: item_port,
            ordinal,
            documents,
            parts,
        })
    }
}

#[cfg(test)]
#[path = "port_selection_tests.rs"]
mod tests;
