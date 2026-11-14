// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_fade::BankMusicFadeFields;
use crate::hirc_music_transition_rule::{self, BankMusicTransitionRuleFields};

/// Compact observations from the original rules of one music node.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MusicTransitionFeatures {
    fades: bool,
    segments: bool,
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

    /// Returns the combined observations of two resolved source nodes.
    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        Self {
            fades: self.fades || other.fades,
            segments: self.segments || other.segments,
        }
    }

    fn observe(&mut self, rule: &BankMusicTransitionRuleFields) {
        let object = rule.transition_object.as_ref();
        self.fades |= fade_timing(&rule.source.fade)
            || fade_timing(&rule.destination.fade)
            || object.is_some_and(|object| {
                fade_timing(&object.fade_in) || fade_timing(&object.fade_out)
            });
        self.segments |= object
            .and_then(|object| object.segment_id.as_ref())
            .is_some_and(|id| id.value != 0);
    }
}

fn fade_timing(fade: &BankMusicFadeFields) -> bool {
    [&fade.time_bits, &fade.offset_bits]
        .into_iter()
        .flatten()
        .any(|word| word.value != 0)
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
    /// Timing and segment features found in readable original rule fields.
    pub features: MusicTransitionFeatures,
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
        observed.features.observe(&rule);
        read?;
        observed.rules_range = Some(start..fields.offset());
        observed.complete_rules = ordinal + 1;
    }
    Ok(())
}
