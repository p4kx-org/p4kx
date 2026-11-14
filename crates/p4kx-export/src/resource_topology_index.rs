// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::SourceUse;

use super::catalog_entity_configurations::EntityConfigurationRows;
use super::catalog_item_alignment::TopologyPortDefinition;
use super::catalog_resource_component::ResourceComponent;
use super::item_component_owner::ComponentOwner;
use super::resource_network_links::ResourceNetworkLinks;
use super::room_associations::RoomAssociations;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(super) struct TopologyIndex<'rows, 'source> {
    records: HashMap<u32, TopologySource<'rows, 'source>>,
    pub(super) attachment_types: HashMap<(u32, u32), &'rows SourceUse>,
}

#[derive(Default)]
pub(super) struct TopologySource<'rows, 'source> {
    pub(super) incomplete_ports: bool,
    pub(super) links: Vec<&'rows ResourceNetworkLinks<'source>>,
    pub(super) rooms: Vec<&'rows RoomAssociations<'source>>,
    pub(super) resources: Vec<&'rows ResourceComponent<'source>>,
    pub(super) ports: Vec<TopologyPortDefinition<'rows>>,
    pub(super) port_inventory_uses: Vec<&'rows SourceUse>,
}

impl TopologySource<'_, '_> {
    pub(super) const fn has_declarations(&self) -> bool {
        !self.links.is_empty() || !self.rooms.is_empty()
    }
}

impl<'rows, 'source> TopologyIndex<'rows, 'source> {
    pub(super) fn new(rows: &'rows EntityConfigurationRows<'source>) -> Result<Self, CatalogError> {
        let mut index = Self {
            records: HashMap::new(),
            attachment_types: rows.alignment.attachment_type_uses().collect(),
        };
        for row in &rows.topology.links {
            index.record(row.owner())?.links.push(row);
        }
        for row in &rows.topology.rooms {
            index.record(row.owner())?.rooms.push(row);
        }
        for row in rows.resources() {
            index.record(row.owner())?.resources.push(row);
        }
        for definition in rows.alignment.topology_ports() {
            let definition = definition?;
            index
                .records
                .entry(definition.position.owner_record_index)
                .or_default()
                .ports
                .push(definition);
        }
        for (record, original) in rows.alignment.port_inventory_uses() {
            index
                .records
                .entry(record)
                .or_default()
                .port_inventory_uses
                .push(original);
        }
        for record in rows.alignment.incomplete_port_records() {
            index.records.entry(record).or_default().incomplete_ports = true;
        }
        for source in index.records.values_mut() {
            source.links.sort_by_key(|row| row.owner().key());
            source.rooms.sort_by_key(|row| row.owner().key());
            source.resources.sort_by_key(|row| row.owner().key());
            source
                .ports
                .sort_by_key(|port| (port.position.component_ordinal, port.position.port_ordinal));
        }
        Ok(index)
    }

    fn record(
        &mut self,
        owner: &ComponentOwner,
    ) -> Result<&mut TopologySource<'rows, 'source>, CatalogError> {
        let record = owner
            .key()
            .0
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(self.records.entry(record).or_default())
    }

    pub(super) fn get(&self, record: u32) -> Option<&TopologySource<'rows, 'source>> {
        self.records.get(&record)
    }

    pub(super) fn resources(&self, record: u32) -> &[&'rows ResourceComponent<'source>] {
        self.get(record).map_or(&[], |source| &source.resources)
    }
}
