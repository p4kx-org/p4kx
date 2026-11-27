// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_fade::MusicFadeTimingField;
use crate::hirc_music_transition_rule::{self, BankMusicTransitionRuleFields};

/// Compact observations from the original rules of one music node.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MusicTransitionFeatures {
    fades: bool,
    segments: bool,
    scheduling: bool,
}

impl MusicTransitionFeatures {
    /// Returns whether any checked fade has nonzero time or offset.
    #[must_use]
    pub const fn has_fade_timing(self) -> bool {
        self.fades
    }

    /// Returns whether any allocated object names a nonzero segment.
    #[must_use]
    pub const fn has_transition_segment(self) -> bool {
        self.segments
    }

    /// Returns whether a checked rule explicitly enables a scheduling control.
    #[must_use]
    pub const fn has_scheduling(self) -> bool {
        self.scheduling
    }

    /// Returns the combined observations of two resolved source nodes.
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        Self {
            fades: self.fades || other.fades,
            segments: self.segments || other.segments,
            scheduling: self.scheduling || other.scheduling,
        }
    }
}

/// The transition rule member that supplies a nonzero fade timing word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicTransitionFadeSide {
    /// Original source rule fade.
    Source,
    /// Original destination rule fade.
    Destination,
    /// Original fade-in on an allocated transition object.
    ObjectFadeIn,
    /// Original fade-out on an allocated transition object.
    ObjectFadeOut,
}

/// The first nonzero fade timing word and its original transition rule context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTransitionFadeEvidence {
    /// Original fade member.
    pub side: MusicTransitionFadeSide,
    /// Original signed field kind.
    pub field: MusicFadeTimingField,
    /// Original signed word bits and byte range.
    pub word: BankHircField<u32>,
    /// Original allocation byte when the word belongs to a transition object.
    pub allocation_flag: Option<BankHircField<u8>>,
}

impl MusicTransitionFadeEvidence {
    fn of(rule: &BankMusicTransitionRuleFields) -> Option<Self> {
        let object = rule.transition_object.as_ref();
        [
            (
                MusicTransitionFadeSide::Source,
                Some(&rule.source.fade),
                None,
            ),
            (
                MusicTransitionFadeSide::Destination,
                Some(&rule.destination.fade),
                None,
            ),
            (
                MusicTransitionFadeSide::ObjectFadeIn,
                object.map(|value| &value.fade_in),
                rule.allocation_flag.as_ref(),
            ),
            (
                MusicTransitionFadeSide::ObjectFadeOut,
                object.map(|value| &value.fade_out),
                rule.allocation_flag.as_ref(),
            ),
        ]
        .into_iter()
        .find_map(|(side, fade, allocation_flag)| {
            let (field, word) = fade?.timing_evidence()?;
            Some(Self {
                side,
                field,
                word: word.clone(),
                allocation_flag: allocation_flag.cloned(),
            })
        })
    }
}

/// Original allocation and segment fields of an omitted transition object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTransitionSegmentEvidence {
    /// Original nonzero allocation byte.
    pub allocation_flag: BankHircField<u8>,
    /// Original nonzero segment reference.
    pub segment_id: BankHircField<u32>,
}

impl MusicTransitionSegmentEvidence {
    fn of(rule: &BankMusicTransitionRuleFields) -> Option<Self> {
        let allocation_flag = rule.allocation_flag.as_ref()?;
        let segment_id = rule.transition_object.as_ref()?.segment_id.as_ref()?;
        (allocation_flag.value != 0 && segment_id.value != 0).then(|| Self {
            allocation_flag: allocation_flag.clone(),
            segment_id: segment_id.clone(),
        })
    }
}

/// The original field that establishes a transition scheduling control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicTransitionSchedulingField {
    /// Plays the source segment's post-exit region.
    SourcePlayPostExit,
    /// Plays the destination segment's pre-entry region.
    DestinationPlayPreEntry,
    /// Matches the destination cue to the source cue name.
    DestinationMatchSourceCueName,
    /// Selects a source exit other than an immediate stop.
    SourceSyncType,
    /// Selects a destination entry other than its entry marker.
    DestinationEntryType,
    /// Selects a destination item other than the playlist start.
    DestinationJumpToType,
}

/// The first checked scheduling control and its original byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTransitionSchedulingEvidence {
    /// The scheduling field selected by the rule.
    pub field: MusicTransitionSchedulingField,
    /// The checked value and its range in the field's original width.
    pub word: BankHircField<u32>,
}

impl MusicTransitionSchedulingEvidence {
    fn of(rule: &BankMusicTransitionRuleFields) -> Option<Self> {
        use MusicTransitionSchedulingField::{
            DestinationEntryType, DestinationJumpToType, DestinationMatchSourceCueName,
            DestinationPlayPreEntry, SourcePlayPostExit, SourceSyncType,
        };

        const ENTRY_SAME_TIME: u16 = 0x1;
        const ENTRY_LAST_EXIT_TIME: u16 = 0x4;
        const JUMP_SPECIFIC_ITEM: u16 = 0x1;
        const JUMP_NEXT_SEGMENT: u16 = 0x3;

        [
            (SourcePlayPostExit, rule.source.play_post_exit.as_ref()),
            (
                DestinationPlayPreEntry,
                rule.destination.play_pre_entry.as_ref(),
            ),
            (
                DestinationMatchSourceCueName,
                rule.destination.match_source_cue_name.as_ref(),
            ),
        ]
        .into_iter()
        .find_map(|(field, flag)| Self::checked(field, flag, |value| value == 1))
        .or_else(|| {
            Self::checked(
                SourceSyncType,
                rule.source.sync_type.as_ref(),
                is_scheduled_music_sync,
            )
        })
        .or_else(|| {
            Self::checked(
                DestinationEntryType,
                rule.destination.entry_type.as_ref(),
                |value| matches!(value, ENTRY_SAME_TIME..=ENTRY_LAST_EXIT_TIME),
            )
        })
        .or_else(|| {
            Self::checked(
                DestinationJumpToType,
                rule.destination.jump_to_type.as_ref(),
                |value| matches!(value, JUMP_SPECIFIC_ITEM..=JUMP_NEXT_SEGMENT),
            )
        })
    }

    fn checked<T: Copy + Into<u32>>(
        field: MusicTransitionSchedulingField,
        original: Option<&BankHircField<T>>,
        applies: impl FnOnce(T) -> bool,
    ) -> Option<Self> {
        let original = original.filter(|word| applies(word.value))?;
        Some(Self {
            field,
            word: BankHircField {
                value: original.value.into(),
                range: original.range.clone(),
            },
        })
    }
}

pub(crate) const fn is_scheduled_music_sync(value: u32) -> bool {
    const SYNC_NEXT_GRID: u32 = 0x1;
    const SYNC_LAST_EXIT_POSITION: u32 = 0x9;
    matches!(value, SYNC_NEXT_GRID..=SYNC_LAST_EXIT_POSITION)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MusicTransitionEvidence {
    pub(crate) fade: Option<MusicTransitionFadeEvidence>,
    pub(crate) segment: Option<MusicTransitionSegmentEvidence>,
    pub(crate) scheduling: Option<MusicTransitionSchedulingEvidence>,
}

impl MusicTransitionEvidence {
    fn observe(&mut self, rule: &BankMusicTransitionRuleFields) -> MusicTransitionFeatures {
        if self.fade.is_none() {
            self.fade = MusicTransitionFadeEvidence::of(rule);
        }
        if self.segment.is_none() {
            self.segment = MusicTransitionSegmentEvidence::of(rule);
        }
        if self.scheduling.is_none() {
            self.scheduling = MusicTransitionSchedulingEvidence::of(rule);
        }
        MusicTransitionFeatures {
            fades: self.fade.is_some(),
            segments: self.segment.is_some(),
            scheduling: self.scheduling.is_some(),
        }
    }

    pub(crate) fn with_bank_offset(self, bank_offset: u64) -> Self {
        Self {
            fade: self.fade.map(|fade| MusicTransitionFadeEvidence {
                word: fade.word.with_bank_offset(bank_offset),
                allocation_flag: fade
                    .allocation_flag
                    .map(|flag| flag.with_bank_offset(bank_offset)),
                ..fade
            }),
            segment: self.segment.map(|segment| MusicTransitionSegmentEvidence {
                allocation_flag: segment.allocation_flag.with_bank_offset(bank_offset),
                segment_id: segment.segment_id.with_bank_offset(bank_offset),
            }),
            scheduling: self
                .scheduling
                .map(|value| MusicTransitionSchedulingEvidence {
                    word: value.word.with_bank_offset(bank_offset),
                    ..value
                }),
        }
    }
}

/// Original transition count and checked physical rule extents.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTransitionFields {
    /// Original rule count.
    pub rule_count: Option<BankHircField<u32>>,
    /// Original complete-rule prefix after the count.
    pub rules_range: Option<Range<u64>>,
    /// Checked rule bytes, including an incomplete final rule.
    pub checked_range: Option<Range<u64>>,
    /// Number of complete original rules.
    pub complete_rules: u32,
    /// Timing, segment and scheduling features found in readable original rule fields.
    pub features: MusicTransitionFeatures,
    pub(crate) evidence: MusicTransitionEvidence,
}

pub(crate) fn read<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankMusicTransitionFields,
) -> Result<u32, F::Error> {
    let count = fields.read_u32()?;
    let rules = count.value;
    let start = fields.offset();
    observed.rule_count = Some(count);
    let result = scan(fields, start, rules, observed);
    let end = if result.is_ok() {
        fields.offset()
    } else {
        fields.offset() + fields.remaining_bytes()
    };
    observed.checked_range = Some(start..end);
    result.map(|()| rules)
}

fn scan<F: ObjectFields>(
    fields: &mut F,
    start: u64,
    count: u32,
    observed: &mut BankMusicTransitionFields,
) -> Result<(), F::Error> {
    observed.rules_range = Some(start..start);
    for ordinal in 0..count {
        let first_count = fields.read_u32()?;
        let (rule, read) = hirc_music_transition_rule::read(fields, first_count);
        observed.features = observed.evidence.observe(&rule);
        read?;
        observed.rules_range = Some(start..fields.offset());
        observed.complete_rules = ordinal + 1;
    }
    Ok(())
}
