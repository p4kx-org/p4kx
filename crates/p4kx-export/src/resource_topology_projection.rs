// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, DataForge, SourceUses, SourceValue, SourcedValue};

use super::catalog_entity_configurations::EntityConfigurationRows;
use super::network_endpoint::{LocalPorts, NetworkEndpoint};
use super::resource_network_placements::{
    CatalogPlacements, FuseEvidence, InstalledResourceComponent, LinkFamily, LoadoutPosition,
    ResourceNetworkLinkBinding, ResourceNetworkNode, ResourceNetworkPlacements,
    RoomAssociationBinding,
};
use super::resource_topology_index::{TopologyIndex, TopologySource};
use crate::collected::Collected;
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::loadout::alignment::AttachmentIdentity;
use crate::{CatalogError, ExportAccountingError};

struct Context<'loadout> {
    position: LoadoutPosition,
    record: Option<u32>,
    ports: &'loadout [LoadoutPort],
    complete: bool,
}

impl Context<'_> {
    fn child(&self, ordinal: u32) -> LoadoutPosition {
        let mut position = self.position.clone();
        position.port_ordinals.push(ordinal);
        position
    }
}

pub(crate) struct CatalogResourceTopology<'source> {
    pub(crate) declarations: super::resource_topology_declarations::ResourceTopologyTables<'source>,
    pub(crate) placements: CatalogPlacements,
}

impl<'source> Collected<'source> {
    pub(crate) fn resource_topology(
        &mut self,
        forge: &DataForge<'_>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<CatalogResourceTopology<'source>, CatalogError> {
        let placements = project(&self.loadouts, &self.entity_configurations, sources, cancel)?
            .into_tables(forge);
        let declarations =
            std::mem::take(&mut self.entity_configurations.topology).into_tables(forge);
        Ok(CatalogResourceTopology {
            declarations,
            placements,
        })
    }
}

fn project(
    loadouts: &[Loadout],
    selected: &EntityConfigurationRows<'_>,
    sources: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<ResourceNetworkPlacements, CatalogError> {
    let index = TopologyIndex::new(selected)?;
    let mut graph = ResourceNetworkPlacements::default();
    for (ordinal, loadout) in loadouts.iter().enumerate() {
        check_cancel(cancel)?;
        if loadout.entity_kind != EntityKind::Ship {
            continue;
        }
        let position = LoadoutPosition {
            loadout_ordinal: to_ordinal(ordinal)?,
            port_ordinals: Vec::new(),
        };
        let record = loadout.root_record_index;
        let root_resources = components(&index, Some(record));
        if !root_resources.is_empty() {
            graph.nodes.push(ResourceNetworkNode {
                position: position.clone(),
                original_port: None,
                attached_item: None,
                vehicle_boundary: None,
                resource_components: root_resources,
                fuse_evidence: FuseEvidence::Unestablished,
            });
        }
        let mut stack = vec![Context {
            position,
            record: Some(record),
            ports: &loadout.ports,
            complete: loadout.port_inventory_complete,
        }];
        while let Some(context) = stack.pop() {
            check_cancel(cancel)?;
            graph.add_context(&context, &index, sources, cancel)?;
            for (ordinal, port) in context.ports.iter().enumerate().rev() {
                let record = target_record(port.source_constraints.attachment());
                let has_declarations = record
                    .and_then(|record| index.get(record))
                    .is_some_and(TopologySource::has_declarations);
                if port.docked_vehicle || (port.ports.is_empty() && !has_declarations) {
                    continue;
                }
                stack.push(Context {
                    position: context.child(to_ordinal(ordinal)?),
                    record,
                    ports: &port.ports,
                    complete: port.attachment_port_inventory_complete,
                });
            }
        }
    }
    Ok(graph)
}

impl ResourceNetworkPlacements {
    fn add_context(
        &mut self,
        context: &Context<'_>,
        index: &TopologyIndex<'_, '_>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let mut referenced = HashSet::new();
        if let Some(record) = context.record
            && let Some(source) = index.get(record)
        {
            let local = LocalPorts::new(
                context.ports,
                &source.ports,
                &source.port_inventory_uses,
                context.complete && !source.incomplete_ports,
                sources,
            )?;
            self.add_bindings(context, source, &local, record, &mut referenced, cancel)?;
        }
        for (ordinal, port) in context.ports.iter().enumerate() {
            check_cancel(cancel)?;
            let ordinal = to_ordinal(ordinal)?;
            let attachment = port.source_constraints.attachment();
            let resource_components = components(index, target_record(attachment));
            if referenced.contains(&ordinal) || !resource_components.is_empty() {
                self.nodes.push(ResourceNetworkNode {
                    position: context.child(ordinal),
                    original_port: port.source_constraints.physical_port()?,
                    attached_item: attachment.cloned(),
                    vehicle_boundary: boundary(port, index, sources)?,
                    resource_components,
                    fuse_evidence: FuseEvidence::Unestablished,
                });
            }
        }
        Ok(())
    }

    fn add_bindings(
        &mut self,
        context: &Context<'_>,
        source: &TopologySource<'_, '_>,
        local: &LocalPorts<'_, '_>,
        record: u32,
        referenced: &mut HashSet<u32>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for declaration in &source.links {
            for link in declaration.hardpoints() {
                check_cancel(cancel)?;
                let port1 = local.name(&link.port1, link.port1_use.as_ref())?;
                let port2 = local.name(&link.port2, link.port2_use.as_ref())?;
                referenced.extend(port_ordinals(&port1).chain(port_ordinals(&port2)));
                self.links.push(ResourceNetworkLinkBinding {
                    context: context.position.clone(),
                    family: LinkFamily::Hardpoint,
                    owner: declaration.owner().clone(),
                    link_ordinal: link.link_ordinal.clone(),
                    port1,
                    port2,
                });
            }
            for link in declaration.resources() {
                check_cancel(cancel)?;
                let port1 = local.weak(record, &link.port1, link.port1_use.as_ref())?;
                let port2 = local.weak(record, &link.port2, link.port2_use.as_ref())?;
                referenced.extend(port_ordinals(&port1).chain(port_ordinals(&port2)));
                self.links.push(ResourceNetworkLinkBinding {
                    context: context.position.clone(),
                    family: LinkFamily::Resource,
                    owner: declaration.owner().clone(),
                    link_ordinal: link.link_ordinal.clone(),
                    port1,
                    port2,
                });
            }
        }
        for declaration in &source.rooms {
            for room in declaration.containers() {
                check_cancel(cancel)?;
                let port =
                    local.name(&room.resource_network_itemport_name, room.port_use.as_ref())?;
                referenced.extend(port_ordinals(&port));
                self.rooms.push(RoomAssociationBinding {
                    context: context.position.clone(),
                    owner: declaration.owner().clone(),
                    container_ordinal: room.container_ordinal.clone(),
                    port,
                });
            }
        }
        Ok(())
    }
}

fn port_ordinals(cell: &SourcedValue<NetworkEndpoint>) -> impl Iterator<Item = u32> + '_ {
    cell.value()
        .into_iter()
        .flat_map(NetworkEndpoint::port_ordinals)
}

fn boundary(
    port: &LoadoutPort,
    index: &TopologyIndex<'_, '_>,
    sources: &SourceUses,
) -> Result<Option<SourcedValue<bool>>, CatalogError> {
    let Some(attachment) = port.source_constraints.attachment_proof() else {
        return Ok(None);
    };
    let type_use = port
        .source_constraints
        .attachment_component()?
        .and_then(|position| index.attachment_types.get(&position));
    let mut inputs = attachment.source_uses.clone();
    inputs.extend(type_use.into_iter().copied().cloned());
    Ok(Some(sources.value(
        SourceValue::Present(port.docked_vehicle),
        &inputs,
        attachment.source_set_complete && type_use.is_some(),
    )))
}

fn target_record(attachment: Option<&AttachmentIdentity>) -> Option<u32> {
    attachment?.target_record_index.value().copied()
}

fn components(
    index: &TopologyIndex<'_, '_>,
    record: Option<u32>,
) -> Vec<InstalledResourceComponent> {
    record
        .into_iter()
        .flat_map(|record| index.resources(record))
        .map(|component| InstalledResourceComponent {
            owner: component.owner().clone(),
            is_relay: component.is_relay().clone(),
        })
        .collect()
}

fn to_ordinal(ordinal: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)
}

fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
