// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{HircValue, MusicTransitionFeatures};

#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum PlaybackFeature {
    MusicFades = 1 << 0,
    MusicTransitionSegments = 1 << 1,
    Loops = 1 << 2,
    Conditions = 1 << 3,
    Automation = 1 << 4,
    ContainerTransitions = 1 << 5,
    BlendCurves = 1 << 6,
    MusicTransitionScheduling = 1 << 7,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlaybackFeatures(u8);

impl PlaybackFeatures {
    pub(super) fn observe(value: &HircValue) -> Self {
        let mut features = Self::default();
        match value {
            HircValue::Container(container) => {
                features.observe_flag(PlaybackFeature::Loops, container.has_authored_loops);
                features.observe_flag(PlaybackFeature::Conditions, container.has_switch_conditions);
                features.observe_flag(
                    PlaybackFeature::ContainerTransitions,
                    container.has_authored_transitions(),
                );
                features.observe_flag(PlaybackFeature::BlendCurves, container.has_blend_curves());
            }
            HircValue::MusicPlaylist(playlist) => {
                features.observe_flag(PlaybackFeature::Loops, playlist.has_authored_loops);
                features.observe_music_transitions(playlist.transition_features);
            }
            HircValue::MusicSwitch(switch) => {
                features.observe_flag(PlaybackFeature::Conditions, switch.has_decision_conditions);
                features.observe_music_transitions(switch.transition_features);
            }
            HircValue::MusicTrack(track) => {
                let has_conditions = track.switch.as_ref().is_some_and(|switch| {
                    switch.default_switch != 0 || !switch.switches.is_empty()
                });
                features.observe_flag(PlaybackFeature::Conditions, has_conditions);
                features.observe_flag(PlaybackFeature::Automation, track.has_automation);
                features.observe_flag(PlaybackFeature::MusicFades, track.has_switch_fades);
                features.observe_flag(
                    PlaybackFeature::MusicTransitionScheduling,
                    track.switch_sync_evidence().is_some(),
                );
            }
            _ => {}
        }
        features
    }

    pub(super) const fn combine(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(super) const fn contains(self, feature: PlaybackFeature) -> bool {
        self.0 & feature as u8 != 0
    }

    const fn observe_flag(&mut self, feature: PlaybackFeature, present: bool) {
        if present {
            self.0 |= feature as u8;
        }
    }

    const fn observe_music_transitions(&mut self, transitions: MusicTransitionFeatures) {
        self.observe_flag(PlaybackFeature::MusicFades, transitions.has_fade_timing());
        self.observe_flag(
            PlaybackFeature::MusicTransitionSegments,
            transitions.has_transition_segment(),
        );
        self.observe_flag(
            PlaybackFeature::MusicTransitionScheduling,
            transitions.has_scheduling(),
        );
    }
}
