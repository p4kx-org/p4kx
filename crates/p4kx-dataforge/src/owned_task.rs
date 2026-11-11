// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout::FieldSlot;
use crate::{DataForgeError, DataType};

use super::positions::{Position, Positions};

/// An original edge retained only for a structural prefix or weak slot.
#[derive(Clone, Copy)]
pub(super) struct Site {
    pub(super) parent: Position,
    pub(super) ordinal: u64,
}

#[derive(Clone, Copy)]
pub(super) enum Input<'source> {
    Field {
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
    },
    Single {
        data_type: DataType,
        target: u16,
        bytes: &'source [u8],
    },
    Owned {
        struct_index: u32,
        instance_index: u32,
    },
}

#[derive(Clone, Copy)]
pub(super) struct Task<'source> {
    pub(super) input: Input<'source>,
    pub(super) site: Option<Site>,
    pub(super) group: Position,
}

impl Task<'_> {
    pub(super) fn position(&self, positions: &mut Positions) -> Result<Position, DataForgeError> {
        let site = self.site.ok_or(DataForgeError::CanonicalTreeInvariant)?;
        positions.append(site.parent, site.ordinal)
    }
}
