// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    MusicTrack, MusicTrackClipTimingKind, MusicTrackClipTimingWord, MusicTrackEvidenceKind,
    MusicTrackEvidenceWord,
};

use super::{DiagnosticSite, DiagnosticWord, TxtpApproximation};

const TRACK: &str = "bnkHircMusicTrack";

pub(super) fn append(sites: &mut Vec<DiagnosticSite>, track: &MusicTrack) {
    let automation = track
        .automation_point_count_evidence()
        .map(|word| DiagnosticWord::new(TRACK, "musicTrackAutomationPointCount", &word.range));
    let posted = track
        .clip_event_evidence()
        .map(|word| DiagnosticWord::new(TRACK, "musicTrackClipEventId", &word.range));
    for (approximation, word) in [
        (TxtpApproximation::OmitMusicTrackAutomation, automation),
        (TxtpApproximation::OmitMusicClipEvents, posted),
        (
            TxtpApproximation::OmitContainerConditions,
            track.switch_condition_evidence().and_then(track_word),
        ),
        (
            TxtpApproximation::OmitMusicTransitionFades,
            track.switch_fade_evidence().and_then(track_word),
        ),
        (
            TxtpApproximation::OmitMusicClipTiming,
            track.clip_timing_evidence().and_then(clip_word),
        ),
        (
            TxtpApproximation::NonfiniteMusicClipTiming,
            track.nonfinite_clip_timing_evidence().and_then(clip_word),
        ),
    ] {
        if let Some(word) = word {
            sites.push(DiagnosticSite::one(approximation, word));
        }
    }
}

fn track_word(evidence: &MusicTrackEvidenceWord) -> Option<DiagnosticWord> {
    let field = match evidence.kind {
        MusicTrackEvidenceKind::DefaultSwitch => "musicTrackDefaultSwitchId",
        MusicTrackEvidenceKind::MemberCount => "musicTrackSwitchMemberCount",
        MusicTrackEvidenceKind::SourceFadeTime => "musicTrackSourceFadeTimeBits",
        MusicTrackEvidenceKind::SourceFadeOffset => "musicTrackSourceFadeOffsetBits",
        MusicTrackEvidenceKind::DestinationFadeTime => "musicTrackDestinationFadeTimeBits",
        MusicTrackEvidenceKind::DestinationFadeOffset => "musicTrackDestinationFadeOffsetBits",
        _ => return None,
    };
    Some(DiagnosticWord::new(TRACK, field, &evidence.original.range))
}

fn clip_word(evidence: &MusicTrackClipTimingWord) -> Option<DiagnosticWord> {
    let field = match evidence.kind {
        MusicTrackClipTimingKind::PlayAt => "musicTrackClipPlayAt",
        MusicTrackClipTimingKind::BeginTrim => "musicTrackClipBeginTrim",
        MusicTrackClipTimingKind::EndTrim => "musicTrackClipEndTrim",
        _ => return None,
    };
    Some(DiagnosticWord::new(TRACK, field, &evidence.original.range))
}
