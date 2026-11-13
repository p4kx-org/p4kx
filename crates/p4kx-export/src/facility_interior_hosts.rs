// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{SourceUse, SourcedValue};

use super::super::catalog_entity_configurations::EntityConfigurationRows;
use super::super::facility_instance::ShipContext;
use super::super::item_component_owner::ComponentOwner;
use super::super::loadout_position::LoadoutPosition;
use super::super::resource_topology_declarations::ItemRoomAssociation;
use super::super::room_associations::{RoomAssociation, RoomAssociations};
use super::Projection;
use super::room_frame::RoomFrame;
use super::types::{InteriorRoomCoverage, InteriorRoomDeclaration, InteriorRoomRef};
use super::walk::RoomReader;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct RoomOwners<'rows, 'source> {
    by_record: HashMap<u32, Vec<RoomDefinition<'rows, 'source>>>,
}

#[derive(Clone, Copy)]
enum RoomDefinition<'rows, 'source> {
    Vehicle(&'rows RoomAssociations<'source>),
    Item(&'rows ItemRoomAssociation<'source>),
}

impl RoomDefinition<'_, '_> {
    const fn owner(&self) -> &ComponentOwner {
        match self {
            Self::Vehicle(room) => room.owner(),
            Self::Item(room) => room.owner(),
        }
    }
}

impl<'rows, 'source> RoomOwners<'rows, 'source> {
    pub(super) fn new(
        declarations: &'rows EntityConfigurationRows<'source>,
    ) -> Result<Self, CatalogError> {
        let mut by_record = HashMap::<u32, Vec<RoomDefinition<'_, '_>>>::new();
        let definitions = declarations
            .topology
            .rooms
            .iter()
            .map(RoomDefinition::Vehicle)
            .chain(
                declarations
                    .topology
                    .item_rooms
                    .iter()
                    .map(RoomDefinition::Item),
            );
        for definition in definitions {
            let (record, _) = definition.owner().key();
            by_record
                .entry(record.ok_or(ExportAccountingError::ConflictingSourceDatum)?)
                .or_default()
                .push(definition);
        }
        for definitions in by_record.values_mut() {
            definitions.sort_by_key(|definition| definition.owner().key());
        }
        Ok(Self { by_record })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.by_record.is_empty()
    }

    pub(super) fn contains(&self, record: u32) -> bool {
        self.by_record.contains_key(&record)
    }

    fn definitions(
        &self,
        record: u32,
    ) -> impl Iterator<Item = RoomDefinition<'rows, 'source>> + '_ {
        self.by_record.get(&record).into_iter().flatten().copied()
    }
}

pub(super) struct RoomAttachment {
    pub(super) record: u32,
    pub(super) ship: ShipContext,
    pub(super) position: SourcedValue<LoadoutPosition>,
    pub(super) selectors: Vec<SourceUse>,
    pub(super) complete: bool,
}

pub(super) struct RoomHost<'host> {
    pub(super) record: u32,
    pub(super) ship: &'host ShipContext,
    pub(super) attachment: Option<&'host RoomAttachment>,
}

pub(super) struct RoomSelection<'source> {
    file: &'source SourceUse,
    attachment: Option<&'source RoomAttachment>,
    pub(super) frame: RoomFrame<'source>,
}

impl RoomSelection<'_> {
    pub(super) fn inputs(&self) -> impl Iterator<Item = &SourceUse> {
        std::iter::once(self.file).chain(
            self.attachment
                .into_iter()
                .flat_map(|attachment| &attachment.selectors),
        )
    }

    pub(super) fn complete(&self) -> bool {
        self.attachment.is_none_or(|attachment| attachment.complete)
    }
}

struct RoomSlot<'row> {
    owner: &'row ComponentOwner,
    declaration: InteriorRoomDeclaration,
    file: Option<&'row str>,
    selector: Option<&'row SourceUse>,
    vehicle: Option<&'row RoomAssociation<'row>>,
}

impl RoomReader<'_> {
    pub(super) fn project_host(
        &self,
        project: &mut Projection<'_, '_>,
        coverage: &mut Vec<InteriorRoomCoverage>,
        rooms: &RoomOwners<'_, '_>,
        host: &RoomHost<'_>,
    ) -> Result<(), CatalogError> {
        for declaration in rooms.definitions(host.record) {
            project.check_cancel()?;
            match declaration {
                RoomDefinition::Vehicle(owner) => {
                    for room in owner.containers() {
                        let slot = RoomSlot {
                            owner: owner.owner(),
                            declaration: InteriorRoomDeclaration::Vehicle {
                                object_container_ordinal: room.container_ordinal.clone(),
                            },
                            file: room.file_name(),
                            selector: room.file_use.as_ref(),
                            vehicle: Some(room),
                        };
                        slot.project(self, project, coverage, host)?;
                    }
                }
                RoomDefinition::Item(room) => {
                    let slot = RoomSlot {
                        owner: room.owner(),
                        declaration: InteriorRoomDeclaration::Item,
                        file: room.file_name(),
                        selector: room.file_use.as_ref(),
                        vehicle: None,
                    };
                    slot.project(self, project, coverage, host)?;
                }
            }
        }
        Ok(())
    }
}

impl RoomSlot<'_> {
    fn project(
        self,
        reader: &RoomReader<'_>,
        project: &mut Projection<'_, '_>,
        coverage: &mut Vec<InteriorRoomCoverage>,
        host: &RoomHost<'_>,
    ) -> Result<(), CatalogError> {
        project.check_cancel()?;
        let Some(path) = self.file.filter(|path| !path.trim().is_empty()) else {
            return Ok(());
        };
        let file = self
            .selector
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let selection = RoomSelection {
            file,
            attachment: host.attachment,
            frame: RoomFrame::read(host.attachment.is_some(), self.vehicle),
        };
        let identity = InteriorRoomRef {
            owner: self.owner.clone(),
            declaration: self.declaration,
            loadout_position: host
                .attachment
                .map(|attachment| attachment.position.clone()),
        };
        coverage.push(reader.room(project, host.ship, identity, &selection, path)?);
        Ok(())
    }
}
