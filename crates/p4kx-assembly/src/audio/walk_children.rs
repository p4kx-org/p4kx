// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::HircValue;

use super::{BreakReason, MAX_CONTAINER_CHILDREN, Via, Walk};

impl Walk<'_, '_> {
    pub(super) fn children(&mut self, ids: &[u32], parent: Option<&HircValue>) {
        for (ordinal, &id) in ids.iter().take(MAX_CONTAINER_CHILDREN).enumerate() {
            let inbound = parent.and_then(|value| {
                self.path
                    .last()?
                    .source
                    .as_ref()?
                    .selection_child(value, ordinal)
            });
            self.reach(id, None, Via::Local, inbound);
        }
        if ids.len() > MAX_CONTAINER_CHILDREN {
            self.broke(BreakReason::TooManyChildren {
                count: ids.len(),
                limit: MAX_CONTAINER_CHILDREN,
            });
        }
    }
}
