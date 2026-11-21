// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUse;

use super::staging::PortContainerFacts;
use super::{ItemAttachmentComponent, ItemPortContainer, PhysicalPort, TopologyPortDefinition};
use crate::CatalogError;

/// In-memory port and attachment facts from a scoped Loadout.
#[derive(Default)]
pub(crate) struct ScopedTopologyAlignment<'source> {
    ports: Vec<PortContainerFacts<'source>>,
    attachments: Vec<ItemAttachmentComponent<'source>>,
}

impl<'source> ScopedTopologyAlignment<'source> {
    pub(crate) fn append_ports(&mut self, rows: Vec<ItemPortContainer<'source>>) {
        self.ports
            .extend(rows.into_iter().map(PortContainerFacts::retain));
    }

    pub(crate) fn append_attachments(&mut self, rows: Vec<ItemAttachmentComponent<'source>>) {
        self.attachments.extend(rows);
    }

    pub(in crate::catalog_v3) fn topology_facts(&self) -> TopologyAlignment<'_, 'source> {
        TopologyAlignment::new(&self.ports, &self.attachments)
    }
}

#[derive(Clone, Copy)]
pub(in crate::catalog_v3) struct TopologyAlignment<'rows, 'source> {
    ports: &'rows [PortContainerFacts<'source>],
    attachments: &'rows [ItemAttachmentComponent<'source>],
}

impl<'rows, 'source> TopologyAlignment<'rows, 'source> {
    pub(super) const fn new(
        ports: &'rows [PortContainerFacts<'source>],
        attachments: &'rows [ItemAttachmentComponent<'source>],
    ) -> Self {
        Self { ports, attachments }
    }

    pub(in crate::catalog_v3) fn topology_ports(
        self,
    ) -> impl Iterator<Item = Result<TopologyPortDefinition<'rows>, CatalogError>> + 'rows {
        self.ports.iter().flat_map(|row| {
            row.ports.iter().filter_map(move |port| {
                let name = port.name?;
                Some(
                    PhysicalPort::read(&row.owner, &port.port_ordinal).map(|position| {
                        TopologyPortDefinition {
                            name,
                            position,
                            name_use: port.name_use.as_ref(),
                        }
                    }),
                )
            })
        })
    }

    pub(in crate::catalog_v3) fn attachment_type_uses(
        self,
    ) -> impl Iterator<Item = ((u32, u32), &'rows SourceUse)> + 'rows {
        self.attachments.iter().filter_map(|row| {
            let (record, component) = row.owner.key();
            let original = row.definition.value()?.type_use.as_ref()?;
            Some(((record?, component?), original))
        })
    }

    pub(in crate::catalog_v3) fn port_inventory_uses(
        self,
    ) -> impl Iterator<Item = (u32, &'rows SourceUse)> + 'rows {
        self.ports
            .iter()
            .filter_map(|row| Some((row.owner.key().0?, row.ports_use.as_ref()?)))
    }

    pub(in crate::catalog_v3) fn incomplete_port_records(
        self,
    ) -> impl Iterator<Item = u32> + 'rows {
        self.ports
            .iter()
            .filter(|row| row.incomplete_names)
            .filter_map(|row| row.owner.key().0)
    }
}
