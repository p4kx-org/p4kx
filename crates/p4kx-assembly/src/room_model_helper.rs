// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Model, Transform};

use crate::entity_interior::lookup;
use crate::placement::{self, NodePlacement};

/// The model node used to anchor a vehicle room.
#[derive(Clone, Copy)]
pub struct RoomModelHelper(NodePlacement);

impl RoomModelHelper {
    /// Selects the first matching model node in decoded order, ignoring ASCII case.
    #[must_use]
    pub fn find(model: &Model, name: &str) -> Option<Self> {
        placement::node_placement(model, name).map(Self)
    }

    /// Returns the selected model node ordinal.
    #[must_use]
    pub const fn node(self) -> usize {
        self.0.node
    }

    /// Returns the helper transform in the owning model frame.
    #[must_use]
    pub const fn transform(self) -> Transform {
        self.0.transform
    }

    /// Returns whether the room's authored offset must follow the helper transform.
    #[must_use]
    pub fn uses_offset(self) -> bool {
        lookup::near(self.transform().translation(), [0.0; 3])
    }
}
