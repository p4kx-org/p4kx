// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A decision made while rendering a resolved event as TXTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TxtpApproximation {
    /// The rendered choice selects its first available alternative.
    SelectFirstAlternative,
    /// The rendered track omits authored nonzero clip start or trim times.
    OmitMusicClipTiming,
    /// The rendered track omits authored Events posted by clips.
    OmitMusicClipEvents,
    /// The owning track contains a nonfinite clip start or trim value.
    NonfiniteMusicClipTiming,
    /// The rendered path omits nonzero authored transition fade timing.
    OmitMusicTransitionFades,
    /// The rendered path omits an authored transition-object segment.
    OmitMusicTransitionSegments,
    /// The rendered path omits an enabled music transition scheduling flag.
    OmitMusicTransitionScheduling,
    /// The rendered path omits authored container or playlist loop controls.
    OmitContainerLoops,
    /// The rendered path omits authored switch or state selection.
    OmitContainerConditions,
    /// The rendered path omits authored `MusicTrack` automation points.
    OmitMusicTrackAutomation,
    /// The rendered path omits a declared Random/Sequence transition mode.
    OmitContainerTransitions,
    /// The rendered path omits authored Blend crossfade curves.
    OmitBlendCurves,
}

impl TxtpApproximation {
    /// Returns the stable code used in reports and TXTP comments.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SelectFirstAlternative => "select-first-alternative",
            Self::OmitMusicClipTiming => "omit-music-clip-timing",
            Self::OmitMusicClipEvents => "omit-music-clip-events",
            Self::NonfiniteMusicClipTiming => "nonfinite-music-clip-timing",
            Self::OmitMusicTransitionFades => "omit-music-transition-fades",
            Self::OmitMusicTransitionSegments => "omit-music-transition-segments",
            Self::OmitMusicTransitionScheduling => "omit-music-transition-scheduling",
            Self::OmitContainerLoops => "omit-container-loops",
            Self::OmitContainerConditions => "omit-container-conditions",
            Self::OmitMusicTrackAutomation => "omit-music-track-automation",
            Self::OmitContainerTransitions => "omit-container-transitions",
            Self::OmitBlendCurves => "omit-blend-curves",
        }
    }
}
