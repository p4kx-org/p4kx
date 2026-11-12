// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::MusicClip;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct MusicClipFeatures {
    pub(super) timing: bool,
    pub(super) nonfinite_timing: bool,
    pub(super) posted_events: bool,
}

impl MusicClipFeatures {
    pub(super) fn observe(clips: &[MusicClip]) -> Self {
        let mut observed = Self::default();
        for clip in clips {
            for value in [clip.play_at, clip.begin_trim, clip.end_trim] {
                if value.is_finite() {
                    observed.timing |= value != 0.0;
                } else {
                    observed.nonfinite_timing = true;
                }
            }
            observed.posted_events |= clip.event_id != 0;
        }
        observed
    }
}
