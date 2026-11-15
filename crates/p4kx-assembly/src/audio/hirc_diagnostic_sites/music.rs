// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    MusicFadeTimingField, MusicPlaylist, MusicPlaylistLoopField, MusicSwitch,
    MusicTransitionFadeEvidence, MusicTransitionFadeSide, MusicTransitionSegmentEvidence,
};

use super::{DiagnosticSite, DiagnosticWord, TxtpApproximation};

const MUSIC_NODE: &str = "bnkHircMusicNode";
const ALLOCATION: &str = "musicTransitionAllocationFlag";

pub(super) fn playlist_sites(sites: &mut Vec<DiagnosticSite>, playlist: &MusicPlaylist) {
    if let Some(evidence) = playlist.authored_loop_evidence() {
        let field = match evidence.field {
            MusicPlaylistLoopField::Count => Some("musicPlaylistLoopCount"),
            MusicPlaylistLoopField::Minimum => Some("musicPlaylistLoopMin"),
            MusicPlaylistLoopField::Maximum => Some("musicPlaylistLoopMax"),
            _ => None,
        };
        if let Some(field) = field {
            sites.push(DiagnosticSite::one(
                TxtpApproximation::OmitContainerLoops,
                DiagnosticWord::new(MUSIC_NODE, field, &evidence.word.range),
            ));
        }
    }
    transitions(
        sites,
        playlist.transition_fade_evidence(),
        playlist.transition_segment_evidence(),
    );
}

pub(super) fn switch_sites(sites: &mut Vec<DiagnosticSite>, switch: &MusicSwitch) {
    if let Some(evidence) = switch.decision_conditions_evidence() {
        sites.push(
            DiagnosticSite::one(
                TxtpApproximation::OmitContainerConditions,
                DiagnosticWord::new(
                    MUSIC_NODE,
                    "musicSwitchGroupCount",
                    &evidence.group_count.range,
                ),
            )
            .with_word(DiagnosticWord::new(
                MUSIC_NODE,
                "musicSwitchTreeSize",
                &evidence.tree_size.range,
            )),
        );
    }
    transitions(
        sites,
        switch.transition_fade_evidence(),
        switch.transition_segment_evidence(),
    );
}

fn transitions(
    sites: &mut Vec<DiagnosticSite>,
    fade: Option<&MusicTransitionFadeEvidence>,
    segment: Option<&MusicTransitionSegmentEvidence>,
) {
    if let Some(fade) = fade.and_then(fade_site) {
        sites.push(fade);
    }
    if let Some(segment) = segment {
        sites.push(
            DiagnosticSite::one(
                TxtpApproximation::OmitMusicTransitionSegments,
                DiagnosticWord::new(
                    MUSIC_NODE,
                    "musicTransitionObjectSegmentId",
                    &segment.segment_id.range,
                ),
            )
            .with_word(DiagnosticWord::new(
                MUSIC_NODE,
                ALLOCATION,
                &segment.allocation_flag.range,
            )),
        );
    }
}

fn fade_site(evidence: &MusicTransitionFadeEvidence) -> Option<DiagnosticSite> {
    let field = match (evidence.side, evidence.field) {
        (MusicTransitionFadeSide::Source, MusicFadeTimingField::Time) => {
            "musicTransitionSourceTimeBits"
        }
        (MusicTransitionFadeSide::Source, MusicFadeTimingField::Offset) => {
            "musicTransitionSourceOffsetBits"
        }
        (MusicTransitionFadeSide::Destination, MusicFadeTimingField::Time) => {
            "musicTransitionDestinationTimeBits"
        }
        (MusicTransitionFadeSide::Destination, MusicFadeTimingField::Offset) => {
            "musicTransitionDestinationOffsetBits"
        }
        (MusicTransitionFadeSide::ObjectFadeIn, MusicFadeTimingField::Time) => {
            "musicTransitionObjectFadeInTimeBits"
        }
        (MusicTransitionFadeSide::ObjectFadeIn, MusicFadeTimingField::Offset) => {
            "musicTransitionObjectFadeInOffsetBits"
        }
        (MusicTransitionFadeSide::ObjectFadeOut, MusicFadeTimingField::Time) => {
            "musicTransitionObjectFadeOutTimeBits"
        }
        (MusicTransitionFadeSide::ObjectFadeOut, MusicFadeTimingField::Offset) => {
            "musicTransitionObjectFadeOutOffsetBits"
        }
        _ => return None,
    };
    let site = DiagnosticSite::one(
        TxtpApproximation::OmitMusicTransitionFades,
        DiagnosticWord::new(MUSIC_NODE, field, &evidence.word.range),
    );
    Some(match &evidence.allocation_flag {
        Some(flag) => site.with_word(DiagnosticWord::new(MUSIC_NODE, ALLOCATION, &flag.range)),
        None => site,
    })
}
