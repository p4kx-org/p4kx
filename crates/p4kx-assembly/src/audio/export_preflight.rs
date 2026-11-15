// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ConversionError;

use super::{BreakReason, TriggerSounds};

pub(super) struct TraversalCut<'a> {
    pub(super) trigger: &'a str,
    pub(super) event: &'a str,
    pub(super) reason: &'a BreakReason,
}

impl TraversalCut<'_> {
    pub(super) fn into_error(self) -> ConversionError {
        ConversionError::IncompleteAudioResolution {
            trigger: self.trigger.to_owned(),
            event: self.event.to_owned(),
            source: Box::new(self.reason.clone()),
        }
    }
}

pub(super) fn traversal_cut<'a>(triggers: &'a [&TriggerSounds<'_>]) -> Option<TraversalCut<'a>> {
    triggers.iter().find_map(|trigger| {
        let cut = trigger.first_truncation()?;
        Some(TraversalCut {
            trigger: trigger.trigger().name(),
            event: cut.event().name(),
            reason: cut.reason(),
        })
    })
}
