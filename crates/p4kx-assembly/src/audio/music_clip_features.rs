// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::MusicTrack;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct MusicClipFeatures {
    pub(super) timing: bool,
    pub(super) nonfinite_timing: bool,
    pub(super) posted_events: bool,
}

impl MusicClipFeatures {
    pub(super) const fn observe(track: &MusicTrack) -> Self {
        Self {
            timing: track.clip_timing_evidence().is_some(),
            nonfinite_timing: track.nonfinite_clip_timing_evidence().is_some(),
            posted_events: track.clip_event_evidence().is_some(),
        }
    }
}
