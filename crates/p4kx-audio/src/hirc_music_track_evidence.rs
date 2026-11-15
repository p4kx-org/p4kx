// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::BankHircField;
use crate::hirc_music::MusicTrack;
use crate::hirc_music_fade::MusicFadeTimingField;

use super::{BankMusicTrackClip, BankMusicTrackSwitchFields};

/// An original `MusicTrack` switch word that proves an authored control.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTrackEvidenceWord {
    /// The kind of original word.
    pub kind: MusicTrackEvidenceKind,
    /// The original word with absolute bank-relative coordinates.
    pub original: BankHircField<u32>,
}

/// A `MusicTrack` switch field relevant to an omitted control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicTrackEvidenceKind {
    /// Nonzero default switch ID.
    DefaultSwitch,
    /// Positive switch member count.
    MemberCount,
    /// Nonzero source fade time.
    SourceFadeTime,
    /// Nonzero source fade offset.
    SourceFadeOffset,
    /// Nonzero destination fade time.
    DestinationFadeTime,
    /// Nonzero destination fade offset.
    DestinationFadeOffset,
}

/// An original binary64 clip timing word relevant to an omitted control.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTrackClipTimingWord {
    /// The kind of original timing word.
    pub kind: MusicTrackClipTimingKind,
    /// The original binary64 bits with absolute bank-relative coordinates.
    pub original: BankHircField<u64>,
}

/// The original timing field within a `MusicTrack` clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicTrackClipTimingKind {
    /// Scheduled play position.
    PlayAt,
    /// Start trim.
    BeginTrim,
    /// End trim.
    EndTrim,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MusicTrackEvidence {
    pub(super) automation_point_count: Option<BankHircField<u32>>,
    pub(super) switch_fade: Option<MusicTrackEvidenceWord>,
    switch_condition: Option<MusicTrackEvidenceWord>,
    clip_timing: Option<MusicTrackClipTimingWord>,
    nonfinite_clip_timing: Option<MusicTrackClipTimingWord>,
    clip_event: Option<BankHircField<u32>>,
}

impl MusicTrack {
    /// Returns the first positive point count in the checked automation table.
    #[must_use]
    pub const fn automation_point_count_evidence(&self) -> Option<&BankHircField<u32>> {
        self.evidence.automation_point_count.as_ref()
    }

    /// Returns the nonzero switch default or the positive checked member count.
    #[must_use]
    pub const fn switch_condition_evidence(&self) -> Option<&MusicTrackEvidenceWord> {
        self.evidence.switch_condition.as_ref()
    }

    /// Returns the first nonzero source or destination fade timing word.
    #[must_use]
    pub const fn switch_fade_evidence(&self) -> Option<&MusicTrackEvidenceWord> {
        self.evidence.switch_fade.as_ref()
    }

    /// Returns the first finite nonzero clip timing word, in clip and field order.
    #[must_use]
    pub const fn clip_timing_evidence(&self) -> Option<&MusicTrackClipTimingWord> {
        self.evidence.clip_timing.as_ref()
    }

    /// Returns the first nonfinite clip timing word, in clip and field order.
    #[must_use]
    pub const fn nonfinite_clip_timing_evidence(&self) -> Option<&MusicTrackClipTimingWord> {
        self.evidence.nonfinite_clip_timing.as_ref()
    }

    /// Returns the first nonzero original clip event ID.
    #[must_use]
    pub const fn clip_event_evidence(&self) -> Option<&BankHircField<u32>> {
        self.evidence.clip_event.as_ref()
    }
}

impl MusicTrackEvidence {
    pub(super) fn observe_clip(&mut self, clip: &BankMusicTrackClip, bank_offset: u64) {
        if self.clip_event.is_none() && clip.event_id.value != 0 {
            self.clip_event = Some(clip.event_id.clone().with_bank_offset(bank_offset));
        }
        if self.clip_timing.is_some() && self.nonfinite_clip_timing.is_some() {
            return;
        }
        for (kind, word) in [
            (MusicTrackClipTimingKind::PlayAt, &clip.play_at),
            (MusicTrackClipTimingKind::BeginTrim, &clip.begin_trim),
            (MusicTrackClipTimingKind::EndTrim, &clip.end_trim),
        ] {
            let value = f64::from_bits(word.value);
            let site = if !value.is_finite() {
                &mut self.nonfinite_clip_timing
            } else if value != 0.0 {
                &mut self.clip_timing
            } else {
                continue;
            };
            site.get_or_insert_with(|| MusicTrackClipTimingWord {
                kind,
                original: word.clone().with_bank_offset(bank_offset),
            });
        }
    }

    pub(super) fn observe_switch(&mut self, switch: &BankMusicTrackSwitchFields, bank_offset: u64) {
        self.switch_condition =
            switch_condition(switch).map(|(kind, word)| MusicTrackEvidenceWord {
                kind,
                original: word.clone().with_bank_offset(bank_offset),
            });
        self.switch_fade = switch_fade(switch).map(|(kind, word)| MusicTrackEvidenceWord {
            kind,
            original: word.clone().with_bank_offset(bank_offset),
        });
    }
}

fn switch_condition(
    switch: &BankMusicTrackSwitchFields,
) -> Option<(MusicTrackEvidenceKind, &BankHircField<u32>)> {
    [
        (
            MusicTrackEvidenceKind::DefaultSwitch,
            switch.default_switch.as_ref(),
        ),
        (
            MusicTrackEvidenceKind::MemberCount,
            switch.member_count.as_ref(),
        ),
    ]
    .into_iter()
    .find_map(|(kind, word)| word.filter(|word| word.value != 0).map(|word| (kind, word)))
}

fn switch_fade(
    switch: &BankMusicTrackSwitchFields,
) -> Option<(MusicTrackEvidenceKind, &BankHircField<u32>)> {
    let transition = switch.transition.as_ref()?;
    if let Some((field, word)) = transition.source.timing_evidence() {
        let kind = match field {
            MusicFadeTimingField::Time => MusicTrackEvidenceKind::SourceFadeTime,
            MusicFadeTimingField::Offset => MusicTrackEvidenceKind::SourceFadeOffset,
        };
        return Some((kind, word));
    }
    let (field, word) = transition.destination.timing_evidence()?;
    let kind = match field {
        MusicFadeTimingField::Time => MusicTrackEvidenceKind::DestinationFadeTime,
        MusicFadeTimingField::Offset => MusicTrackEvidenceKind::DestinationFadeOffset,
    };
    Some((kind, word))
}
