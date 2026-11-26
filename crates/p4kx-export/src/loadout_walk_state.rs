// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Enumerate;
use std::slice::Iter;

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue};

use super::Projection;
use super::RoomLookup;
use super::hosts::RoomAttachment;
use super::installed::InstalledConfigurations;
use crate::CatalogError;
use crate::catalog_v3::facility_instance::ShipContext;
use crate::catalog_v3::loadout_position::{self, LoadoutPosition};
use crate::loadout::alignment::AttachmentProof;
use crate::loadout::join::JoinPort;

struct Frame<'loadout, Port> {
    ports: Enumerate<Iter<'loadout, Port>>,
    parent: Checkpoint,
}

impl<'loadout, Port> Frame<'loadout, Port> {
    fn new(ports: &'loadout [Port], parent: Checkpoint) -> Self {
        Self {
            ports: ports.iter().enumerate(),
            parent,
        }
    }
}

pub(super) struct Path {
    position: LoadoutPosition,
    uses: Vec<SourceUse>,
    complete: bool,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    ports: usize,
    uses: usize,
    complete: bool,
}

impl Path {
    pub(super) fn room_attachment(
        &self,
        record: u32,
        ship: &ShipContext,
        sources: &SourceUses,
    ) -> RoomAttachment {
        RoomAttachment {
            record,
            ship: ship.clone(),
            position: self.position_cell(sources),
            selectors: self.uses.clone(),
            complete: self.complete,
        }
    }
    pub(super) fn position_cell(&self, sources: &SourceUses) -> SourcedValue<LoadoutPosition> {
        sources.value(
            SourceValue::Present(self.position.clone()),
            &self.uses,
            self.complete,
        )
    }

    pub(super) fn new(loadout_record_index: u32, root: SourceUse) -> Self {
        Self {
            position: LoadoutPosition::root(loadout_record_index),
            uses: vec![root],
            complete: true,
        }
    }

    fn enter(
        &mut self,
        ordinal: usize,
        port: &impl JoinPort,
        proof: Option<&AttachmentProof>,
    ) -> Result<(), CatalogError> {
        self.position
            .port_ordinals
            .push(loadout_position::ordinal(ordinal)?);
        if let Some(proof) = proof {
            self.uses.extend_from_slice(&proof.source_uses);
            self.complete &= proof.source_set_complete;
        }
        self.complete &= port.append_name_uses(&mut self.uses)?;
        Ok(())
    }

    const fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            ports: self.position.port_ordinals.len(),
            uses: self.uses.len(),
            complete: self.complete,
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.position.port_ordinals.truncate(checkpoint.ports);
        self.uses.truncate(checkpoint.uses);
        self.complete = checkpoint.complete;
    }
}

impl Projection<'_, '_> {
    pub(super) fn walk_ports(
        &mut self,
        ports: &[impl JoinPort],
        ship: &ShipContext,
        mut path: Path,
        rooms: &mut RoomLookup<'_>,
        room_attachments: &mut Vec<RoomAttachment>,
        flight: &mut InstalledConfigurations<'_>,
    ) -> Result<(), CatalogError> {
        let mut stack = vec![Frame::new(ports, path.checkpoint())];
        while let Some(frame) = stack.last_mut() {
            self.check_cancel()?;
            let Some((ordinal, port)) = frame.ports.next() else {
                path.restore(frame.parent);
                stack.pop();
                continue;
            };
            let state = port.state();
            if state.docked_vehicle || state.unresolved {
                continue;
            }
            let parent = path.checkpoint();
            if let Some((attachment, proof)) = port.alignment().placement_attachment() {
                let Some(record) = attachment.target_record_index.value().copied() else {
                    continue;
                };
                let has_rooms = rooms(record)?;
                let configurations = self.configuration_links(record)?;
                let flight_links = flight.links(record)?;
                if port.children().is_empty()
                    && !has_rooms
                    && configurations.is_empty()
                    && flight_links.is_empty()
                {
                    continue;
                }
                path.enter(ordinal, port, Some(proof))?;
                let attached = Some((port.alignment(), attachment));
                self.emit(ship, &path, attached, configurations)?;
                flight.emit(
                    ship,
                    flight_links,
                    || path.position_cell(&self.origins.uses),
                    attached,
                )?;
                if has_rooms {
                    room_attachments.push(path.room_attachment(record, ship, &self.origins.uses));
                }
            } else if !port.children().is_empty() && !state.has_item_selector() {
                path.enter(ordinal, port, None)?;
            } else {
                continue;
            }
            if port.children().is_empty() {
                path.restore(parent);
            } else {
                stack.push(Frame::new(port.children(), parent));
            }
        }
        Ok(())
    }
}
