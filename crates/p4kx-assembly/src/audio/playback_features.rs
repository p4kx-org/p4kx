// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{HircValue, MusicTransitionFeatures};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlaybackFeatures {
    pub(super) loops: bool,
    pub(super) conditions: bool,
    pub(super) automation: bool,
    pub(super) music_transitions: MusicTransitionFeatures,
}

impl PlaybackFeatures {
    pub(super) fn observe(value: &HircValue) -> Self {
        match value {
            HircValue::Container(container) => Self {
                loops: container.has_authored_loops,
                conditions: container.has_switch_conditions,
                ..Self::default()
            },
            HircValue::MusicPlaylist(playlist) => Self {
                loops: playlist.has_authored_loops,
                music_transitions: playlist.transition_features,
                ..Self::default()
            },
            HircValue::MusicSwitch(switch) => Self {
                conditions: switch.has_decision_conditions,
                music_transitions: switch.transition_features,
                ..Self::default()
            },
            HircValue::MusicTrack(track) => Self {
                conditions: track.switch.as_ref().is_some_and(|switch| {
                    switch.default_switch != 0 || !switch.switches.is_empty()
                }),
                automation: track.has_automation,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    pub(super) const fn combine(self, other: Self) -> Self {
        Self {
            loops: self.loops || other.loops,
            conditions: self.conditions || other.conditions,
            automation: self.automation || other.automation,
            music_transitions: self.music_transitions.combine(other.music_transitions),
        }
    }
}
