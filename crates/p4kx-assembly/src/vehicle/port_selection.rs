// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::aggregate_evidence::Checkpoint;
use super::{VehicleAggregateEvidence, VehicleImplementation, VehicleItemPort, VehicleXmlSite};
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
    port: &'vehicle VehicleItemPort,
    documents: Option<Prefix<'vehicle>>,
    parts: Option<Prefix<'vehicle>>,
}

impl<'vehicle> VehiclePortSelection<'vehicle> {
    /// Borrows the selected item port.
    #[must_use]
    pub const fn port(&self) -> &'vehicle VehicleItemPort {
        self.port
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
}

impl VehicleImplementation {
    /// Returns the first item port of the given name with its optional original XML selectors.
    #[must_use]
    pub fn item_port_with_selection(&self, name: &str) -> Option<VehiclePortSelection<'_>> {
        let indexed = self.port_paths.get(&port_key(name))?;
        let port = self.part_at(&indexed.path)?.item_port()?;
        let origins = &self.declarations;
        let documents = origins
            .aggregate
            .as_ref()
            .zip(origins.port_documents)
            .map(|(evidence, end)| Prefix { evidence, end });
        let parts = origins
            .port_parts
            .as_ref()
            .zip(indexed.prefix)
            .map(|(evidence, end)| Prefix { evidence, end });
        Some(VehiclePortSelection {
            port,
            documents,
            parts,
        })
    }
}
