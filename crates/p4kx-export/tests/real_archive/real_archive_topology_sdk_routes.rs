// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{Attachment, EntityNode, PortNode};
use p4kx_export::catalog_queries::equipment::AlignmentDependencyState;
use p4kx_export::catalog_queries::equipment::resource_topology::{
    LoadoutPosition, ResourceNetworkNode, ResourceTopologyDependencyRole as Role,
    ResourceTopologyPlacementSnapshot,
};
use p4kx_extraction::{DataForge, DataForgeOrigin, TreeBuilder};
use serde_json::json;

use super::super::{Ship, TestResult, relay_slots};
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_catalog_table_parity::wire_row;

pub(super) fn check(
    ship: &Ship<'_>,
    snapshot: &ResourceTopologyPlacementSnapshot,
    original: &DataForge<'_>,
    origin: &DataForgeOrigin,
) -> TestResult {
    check_roles(ship, snapshot)?;
    for row in snapshot.placements.resource_network_nodes().rows {
        let Some(port) = port_at(ship, row.position())? else {
            if row.attached_item().is_some() || row.vehicle_boundary().is_some() {
                return Err("a root resource node has attachment evidence".into());
            }
            continue;
        };
        check_attachment(row, port)?;
    }
    for row in snapshot.placements.resource_network_link_bindings().rows {
        check_owner(
            ship,
            row.context(),
            row.owner().record_index().value().copied(),
        )?;
    }
    for row in snapshot.placements.room_association_bindings().rows {
        check_owner(
            ship,
            row.context(),
            row.owner().record_index().value().copied(),
        )?;
    }
    check_relays(ship, snapshot, original, origin)
}

fn check_attachment(row: &ResourceNetworkNode, port: &PortNode) -> TestResult {
    if let Some(published) = row.original_port() {
        let original = port
            .definition()
            .ok_or("the original port has no physical definition")?
            .position();
        if published.owner_record_index() != original.record_index()
            || usize::try_from(published.component_ordinal())? != original.component_ordinal()
            || usize::try_from(published.port_ordinal())? != original.port_ordinal()
        {
            return Err("a resource node refers to a different original port definition".into());
        }
    }
    let entity = match port.attachment() {
        Attachment::Entity(entity) => Some(entity.as_ref()),
        _ => None,
    };
    let original_target = entity.and_then(EntityNode::record_index);
    let actual_target = row
        .attached_item()
        .and_then(|item| item.target_record_index().value().copied());
    if original_target != actual_target {
        return Err("a resource node differs from the original physical attachment".into());
    }
    if let Some(entity) = entity
        && row
            .vehicle_boundary()
            .and_then(p4kx_extraction::SourcedValue::value)
            != Some(&entity.is_docked_vehicle())
    {
        return Err("an installed resource node differs from the original vehicle boundary".into());
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Roles {
    resources: bool,
    links: bool,
    rooms: bool,
}

impl Roles {
    const fn attach(&mut self, docked: bool) {
        self.resources = true;
        self.links |= !docked;
        self.rooms |= !docked;
    }

    fn observe(&mut self, role: Role) -> TestResult {
        let seen = match role {
            Role::ResourceComponents => &mut self.resources,
            Role::LinkDeclarations => &mut self.links,
            Role::RoomDeclarations => &mut self.rooms,
            _ => return Err("topology reported an unverified declaration role".into()),
        };
        if std::mem::replace(seen, true) {
            return Err("topology reported the same physical declaration role twice".into());
        }
        Ok(())
    }
}

fn check_roles(ship: &Ship<'_>, snapshot: &ResourceTopologyPlacementSnapshot) -> TestResult {
    let mut roots = BTreeMap::new();
    roots
        .entry(ship.tree.record_index())
        .or_insert_with(Roles::default)
        .attach(false);
    ship.assembly.visit_ports(|_, route| {
        if let Some(port) = route.last()
            && let Attachment::Entity(entity) = port.attachment()
            && let Some(record) = entity.record_index()
        {
            roots
                .entry(record)
                .or_default()
                .attach(entity.is_docked_vehicle());
        }
    });
    let mut actual = BTreeMap::new();
    for dependency in &snapshot.declaration_dependencies {
        actual
            .entry(dependency.record_index)
            .or_insert_with(Roles::default)
            .observe(dependency.role)?;
    }
    if roots != actual {
        return Err(
            "typed topology dependency scopes differ from the original Assembly closure".into(),
        );
    }
    Ok(())
}

pub(super) fn check_declaration_reads(
    snapshot: &ResourceTopologyPlacementSnapshot,
    root: u32,
) -> TestResult {
    for dependency in &snapshot.declaration_dependencies {
        if !matches!(
            dependency.state,
            AlignmentDependencyState::Complete | AlignmentDependencyState::NotAdmitted
        ) {
            return Err(
                "a reached declaration role is missing, unreadable or incompletely projected"
                    .into(),
            );
        }
        if dependency.record_index == root && dependency.state != AlignmentDependencyState::Complete
        {
            return Err("an admitted Ship root has a non-complete declaration role".into());
        }
    }
    Ok(())
}

fn port_at<'ship>(
    ship: &'ship Ship<'_>,
    position: &LoadoutPosition,
) -> TestResult<Option<&'ship PortNode>> {
    if position.loadout_record_index() != ship.tree.record_index() {
        return Err("a placement refers to another physical Loadout root".into());
    }
    let mut ports = ship.assembly.ports();
    let mut last = None;
    for &index in position.port_ordinals() {
        let port = ports
            .get(usize::try_from(index)?)
            .ok_or("a placement port route is outside the original Loadout")?;
        last = Some(port);
        ports = match port.attachment() {
            Attachment::Entity(entity) if !entity.is_docked_vehicle() => entity.ports(),
            Attachment::Empty => port.nested(),
            Attachment::Entity(_) | Attachment::Unresolved(_) => &[],
        };
    }
    Ok(last)
}

fn check_owner(ship: &Ship<'_>, position: &LoadoutPosition, actual: Option<u32>) -> TestResult {
    let expected = match port_at(ship, position)? {
        None => Some(ship.tree.record_index()),
        Some(port) => match port.attachment() {
            Attachment::Entity(entity) if !entity.is_docked_vehicle() => entity.record_index(),
            _ => {
                return Err(
                    "a placement binding declared a role past its original owner scope".into(),
                );
            }
        },
    };
    if expected.is_none() || expected != actual {
        return Err("a binding points to a different original declaration owner".into());
    }
    Ok(())
}

fn check_relays(
    ship: &Ship<'_>,
    snapshot: &ResourceTopologyPlacementSnapshot,
    original: &DataForge<'_>,
    origin: &DataForgeOrigin,
) -> TestResult {
    let evidence = json!({"provenance": &snapshot.source_inventory});
    let origins = Origins::read(&evidence, origin)?;
    let mut trees = TreeBuilder::new();
    for (&ordinal, &target) in &ship.relays {
        let ordinal = u32::try_from(ordinal)?;
        let mut selected = snapshot
            .placements
            .resource_network_nodes()
            .rows
            .iter()
            .filter(|node| node.position().port_ordinals() == [ordinal]);
        let placement = selected
            .next()
            .ok_or("original direct relay placement is missing")?;
        if selected.next().is_some() {
            return Err("an original direct relay occurrence was duplicated".into());
        }
        let record = original
            .record(target)
            .ok_or("original relay Record is missing")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read the original attached relay Record")?;
        let positions = Locations::read(tree.root());
        for (slot, raw) in relay_slots(&tree) {
            let slot = u32::try_from(slot)?;
            let original = placement
                .resource_components()
                .iter()
                .find(|component| {
                    component.owner().record_index().value() == Some(&target)
                        && component.owner().component_ordinal().value() == Some(&slot)
                })
                .ok_or("an original direct relay component slot is missing")?;
            let cell = original.is_relay();
            if cell.value() != Some(&true) {
                return Err("the original direct relay classifier differs".into());
            }
            origins.occurrence(&wire_row(cell)?, target, positions.ordinal(raw)?, "$value")?;
        }
    }
    Ok(())
}
