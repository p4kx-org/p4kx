// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Enumerate;
use std::slice::Iter;

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue};

use super::hosts::RoomAttachment;
use crate::catalog_v3::facility_instance::ShipContext;
use crate::catalog_v3::loadout_position::LoadoutPosition;
use crate::contract_loadout::LoadoutPort;

pub(super) struct Frame<'loadout> {
    pub(super) ports: Enumerate<Iter<'loadout, LoadoutPort>>,
    pub(super) parent: Checkpoint,
}

impl<'loadout> Frame<'loadout> {
    pub(super) fn new(ports: &'loadout [LoadoutPort], path: &Path) -> Self {
        Self {
            ports: ports.iter().enumerate(),
            parent: path.checkpoint(),
        }
    }
}

pub(super) struct Path {
    pub(super) position: LoadoutPosition,
    pub(super) uses: Vec<SourceUse>,
    pub(super) complete: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
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

    pub(super) const fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            ports: self.position.port_ordinals.len(),
            uses: self.uses.len(),
            complete: self.complete,
        }
    }

    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        self.position.port_ordinals.truncate(checkpoint.ports);
        self.uses.truncate(checkpoint.uses);
        self.complete = checkpoint.complete;
    }
}
