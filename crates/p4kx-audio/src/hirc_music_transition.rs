// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_transition_rule;

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
        hirc_music_transition_rule::read(fields, first_count).1?;
        observed.rules_range = Some(start..fields.offset());
        observed.complete_rules = ordinal + 1;
    }
    Ok(())
}
