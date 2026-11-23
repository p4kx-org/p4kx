// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use crate::CatalogError;
use crate::catalog_v3::catalog_entity_configurations::{
    ResourceTopologyDeclarations, ResourceTopologyKind, read_loadout_facility_owners,
    read_resource_topology,
};
use crate::catalog_v3::facility_loadout_placements::{
    ClassMembership, loadout_configuration_links,
};
use crate::loadout::alignment::direct::{AlignmentDependencyState, DirectAlignment};

type State = AlignmentDependencyState;

/// One physical declaration or class dependency reached by the selected room scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InteriorCoverageDependency {
    /// Record index in the original DataForge.
    pub record_index: u32,
    /// Outcome of the vehicle-room read at this Record, when consulted.
    pub vehicle_rooms: Option<State>,
    /// Outcome of the item-room read at this Record, when consulted.
    pub item_rooms: Option<State>,
    /// Outcome of the facility-class read at this Record, when consulted.
    pub facility_class: Option<State>,
}

impl InteriorCoverageDependency {
    pub(super) fn is_complete(self) -> bool {
        [self.vehicle_rooms, self.item_rooms, self.facility_class]
            .into_iter()
            .flatten()
            .all(complete)
    }
}

const fn complete(state: State) -> bool {
    matches!(state, State::Complete | State::NotAdmitted)
}

#[derive(Default)]
pub(super) struct RoomReads<'source> {
    pub(super) declarations: ResourceTopologyDeclarations<'source>,
    owners: BTreeMap<u32, RoomRead>,
}

struct RoomRead {
    vehicle: State,
    item: State,
    has_rooms: bool,
}

impl<'source> RoomReads<'source> {
    pub(super) fn read(
        &mut self,
        index: u32,
        alignment: &mut DirectAlignment<'source>,
    ) -> Result<bool, CatalogError> {
        let slot = match self.owners.entry(index) {
            Entry::Occupied(slot) => return Ok(slot.get().has_rooms),
            Entry::Vacant(slot) => slot,
        };
        let source =
            alignment.read_configuration(index, |tree, projection, admission, cancel| {
                let vehicle = read_resource_topology(
                    tree,
                    projection,
                    admission,
                    ResourceTopologyKind::VehicleRooms,
                    cancel,
                )?;
                let item = read_resource_topology(
                    tree,
                    projection,
                    admission,
                    ResourceTopologyKind::ItemRooms,
                    cancel,
                )?;
                let states = (
                    State::from_scoped_read(&vehicle),
                    State::from_scoped_read(&item),
                );
                let mut declarations = vehicle.value;
                declarations.extend(item.value);
                Ok((declarations, states.0, states.1))
            })?;
        let read = match source {
            Ok((declarations, vehicle, item)) => {
                let has_rooms = declarations.has_room_declarations();
                self.declarations.extend(declarations);
                RoomRead {
                    vehicle,
                    item,
                    has_rooms,
                }
            }
            Err(state) => RoomRead {
                vehicle: state,
                item: state,
                has_rooms: false,
            },
        };
        Ok(slot.insert(read).has_rooms)
    }
}

#[derive(Default)]
pub(super) struct ClassReads {
    classes: BTreeMap<u32, (bool, State)>,
}

impl ClassReads {
    pub(super) fn read(
        &mut self,
        index: u32,
        alignment: &mut DirectAlignment<'_>,
    ) -> Result<ClassMembership, CatalogError> {
        let (configured, state) = match self.classes.entry(index) {
            Entry::Occupied(slot) => *slot.get(),
            Entry::Vacant(slot) => {
                let read = alignment.read_configuration(
                    index,
                    |tree, projection, admission, cancel| {
                        let read =
                            read_loadout_facility_owners(tree, projection, admission, cancel)?;
                        let state = State::from_scoped_read(&read);
                        let configured =
                            !loadout_configuration_links(index, &read.value, cancel)?.is_empty();
                        Ok((configured, state))
                    },
                )?;
                *slot.insert(read.unwrap_or_else(|state| (false, state)))
            }
        };
        Ok(ClassMembership {
            configured,
            complete: complete(state),
        })
    }
}

pub(super) fn dependencies(
    rooms: RoomReads<'_>,
    classes: ClassReads,
) -> Vec<InteriorCoverageDependency> {
    let mut captured: BTreeMap<_, _> = rooms
        .owners
        .into_iter()
        .map(|(index, read)| {
            (
                index,
                InteriorCoverageDependency {
                    record_index: index,
                    vehicle_rooms: Some(read.vehicle),
                    item_rooms: Some(read.item),
                    facility_class: None,
                },
            )
        })
        .collect();
    for (index, (_, state)) in classes.classes {
        captured
            .entry(index)
            .or_insert(InteriorCoverageDependency {
                record_index: index,
                vehicle_rooms: None,
                item_rooms: None,
                facility_class: None,
            })
            .facility_class = Some(state);
    }
    captured.into_values().collect()
}
