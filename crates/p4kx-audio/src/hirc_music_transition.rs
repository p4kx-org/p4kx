// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_node::{self, ID_LEN};
use crate::{BankError, SectionId};

const RULE_SOURCE_LEN: usize = 21;
const RULE_DESTINATION_LEN: usize = 26;
const RULE_OBJECT_LEN: usize = 30;
const MIN_RULE_LEN: usize = 2 * ID_LEN + RULE_SOURCE_LEN + RULE_DESTINATION_LEN + 1;

/// Original transition count and the checked extent of complete original rules.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTransitionFields {
    /// Original rule count.
    pub rule_count: Option<BankHircField<u32>>,
    /// Original complete-rule prefix after the count.
    pub rules_range: Option<Range<u64>>,
}

pub(crate) fn read<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankMusicTransitionFields,
) -> Result<u32, F::Error> {
    let count = fields.read_u32()?;
    let rules = count.value;
    observed.rule_count = Some(count);
    if u64::from(rules) > fields.remaining_bytes() / MIN_RULE_LEN as u64 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "a music node lists more transition rules than its body holds",
        )
        .into());
    }
    let start = fields.offset();
    observed.rules_range = Some(start..start);
    for _ in 0..rules {
        skip_rule(fields)?;
        observed.rules_range = Some(start..fields.offset());
    }
    Ok(rules)
}

fn skip_rule<F: ObjectFields>(fields: &mut F) -> Result<(), F::Error> {
    for _ in 0..2 {
        let ids = fields.read_u32()?;
        hirc_node::skip_items(fields, ids.value, ID_LEN)?;
    }
    fields.skip_bytes((RULE_SOURCE_LEN + RULE_DESTINATION_LEN) as u64)?;
    if fields.read_u8()?.value != 0 {
        fields.skip_bytes(RULE_OBJECT_LEN as u64)?;
    }
    Ok(())
}
