// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{TriggerHit, Unmatched};

use super::event::MAX_EVENTS;

/// Owned trigger names and search decisions carried to the Forge worker.
pub(super) struct HitPlan {
    pub(super) name: String,
    pub(super) matched_trigger: bool,
    pub(super) unmatched: Option<Unmatched>,
    pub(super) matched_events: Vec<bool>,
}

impl HitPlan {
    pub(super) fn read(hit: &TriggerHit<'_>) -> Self {
        Self {
            name: hit.trigger().name().to_owned(),
            matched_trigger: hit.matched_by_trigger(),
            unmatched: hit.unmatched(),
            matched_events: hit
                .events()
                .take(MAX_EVENTS)
                .map(|(_, matched)| matched)
                .collect(),
        }
    }
}
