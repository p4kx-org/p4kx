// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::CancelToken;

use super::super::{ResourceTopologyDependency, ResourceTopologyDependencyRole as Role};
use crate::CatalogError;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations::{
    self, ResourceComponent, ResourceTopologyDeclarations, ResourceTopologyKind,
};
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::catalog_v3::resource_topology_projection;
use crate::contract_loadout::Loadout;
use crate::loadout::alignment::direct::{AlignmentDependencyState, DirectAlignment};

#[derive(Default)]
pub(super) struct Originals<'source> {
    pub(super) declarations: ResourceTopologyDeclarations<'source>,
    pub(super) resources: Vec<ResourceComponent<'source>>,
    pub(super) dependencies: Vec<ResourceTopologyDependency>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Context {
    Docked,
    Declaring,
}

pub(super) fn read<'source>(
    root: &Loadout,
    alignment: &mut DirectAlignment<'source>,
    cancel: Option<&CancelToken>,
) -> Result<Originals<'source>, CatalogError> {
    let mut originals = Originals::default();
    for (index, context) in reached(root, cancel)? {
        checkpoint(cancel)?;
        let source =
            alignment.read_configuration(index, |tree, projection, admission, cancel| {
                let resources = catalog_entity_configurations::read_resource_components(
                    tree,
                    projection,
                    admission,
                    &AlignmentOptions::default(),
                    cancel,
                )?;
                let mut record = Originals::default();
                record.resources = record.capture(index, Role::ResourceComponents, resources);
                if context == Context::Declaring {
                    for (kind, role) in DECLARATIONS {
                        let read = catalog_entity_configurations::read_resource_topology(
                            tree, projection, admission, kind, cancel,
                        )?;
                        let declarations = record.capture(index, role, read);
                        record.declarations.extend(declarations);
                    }
                }
                Ok(record)
            })?;
        match source {
            Ok(record) => {
                if record.declarations.has_context_rows() {
                    alignment.include_port_inventory(index)?;
                }
                originals.extend(record);
            }
            Err(state) => originals.unavailable(index, context, state),
        }
    }
    Ok(originals)
}

const DECLARATIONS: [(ResourceTopologyKind, Role); 2] = [
    (ResourceTopologyKind::Links, Role::LinkDeclarations),
    (ResourceTopologyKind::VehicleRooms, Role::RoomDeclarations),
];

fn reached(
    root: &Loadout,
    cancel: Option<&CancelToken>,
) -> Result<BTreeMap<u32, Context>, CatalogError> {
    let mut records = BTreeMap::from([(root.record_index(), Context::Declaring)]);
    let mut pending = vec![root.ports.as_slice()];
    while let Some(ports) = pending.pop() {
        for port in ports {
            checkpoint(cancel)?;
            let context = if port.docked_vehicle {
                Context::Docked
            } else {
                Context::Declaring
            };
            if let Some(index) = resource_topology_projection::attachment_record(port) {
                records
                    .entry(index)
                    .and_modify(|seen| *seen = (*seen).max(context))
                    .or_insert(context);
            }
            if context == Context::Declaring && !port.ports.is_empty() {
                pending.push(&port.ports);
            }
        }
    }
    Ok(records)
}

impl Originals<'_> {
    fn capture<T>(&mut self, index: u32, role: Role, read: ScopedSourceRead<T>) -> T {
        self.dependencies.push(dependency(
            index,
            role,
            AlignmentDependencyState::from_scoped_read(&read),
        ));
        read.value
    }

    fn extend(&mut self, other: Self) {
        self.declarations.extend(other.declarations);
        self.resources.extend(other.resources);
        self.dependencies.extend(other.dependencies);
    }

    fn unavailable(&mut self, index: u32, context: Context, state: AlignmentDependencyState) {
        self.dependencies
            .push(dependency(index, Role::ResourceComponents, state));
        if context == Context::Declaring {
            self.dependencies
                .extend(DECLARATIONS.map(|(_, role)| dependency(index, role, state)));
        }
    }
}

const fn dependency(
    index: u32,
    role: Role,
    state: AlignmentDependencyState,
) -> ResourceTopologyDependency {
    ResourceTopologyDependency {
        record_index: index,
        role,
        state,
    }
}
