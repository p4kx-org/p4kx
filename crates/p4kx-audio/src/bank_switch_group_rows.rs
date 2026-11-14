// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_switch_container::{BankSwitchExtent, WORD_LEN};
use crate::read_at::SeekSource;

/// One physical Switch membership group and its checked member extent.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSwitchGroup {
    /// Zero-based ordinal in the authored group table.
    pub ordinal: u32,
    /// Original switch or state value ID.
    pub value_id: BankHircField<u32>,
    /// Original member count when the entire word fits.
    pub member_count: Option<BankHircField<u32>>,
    /// Checked original member bytes, including any incomplete final ID.
    pub members_range: Option<Range<u64>>,
    /// Checked original group bytes, including any incomplete final field.
    pub checked_range: Range<u64>,
    /// Whether the complete original group and its members fit.
    pub complete: bool,
}

/// One physical membership ID within an authored Switch group.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSwitchMember {
    /// Zero-based ordinal of the owning group.
    pub group_ordinal: u32,
    /// Original value ID range of the owning group.
    pub group_value_range: Range<u64>,
    /// Zero-based ordinal in the authored member list.
    pub ordinal: u32,
    /// Original child reference ID.
    pub child_id: BankHircField<u32>,
}

/// A Switch group followed by the checked original member rows.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankSwitchMembership {
    /// Original value and member-table extent.
    Group(BankSwitchGroup),
    /// Original child reference within the preceding group.
    Member(BankSwitchMember),
}

/// Fixed-memory cursor over groups and members in their original order.
pub struct BankSwitchGroupRows<'input, R> {
    input: SeekObjectFields<'input, R>,
    groups: Range<u32>,
    members: Range<u32>,
    current_group: u32,
    current_group_start: u64,
    stopped: &'input mut bool,
}

impl<'input, R> BankSwitchGroupRows<'input, R> {
    pub(crate) const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        extent: &BankSwitchExtent,
    ) -> Self {
        let range = &extent.checked_range;
        Self {
            input: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            groups: 0..extent.declared_count.value,
            members: 0..0,
            current_group: 0,
            current_group_start: 0,
            stopped,
        }
    }
}

impl<R: Read + Seek> Iterator for BankSwitchGroupRows<'_, R> {
    type Item = Result<BankSwitchMembership, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.input.remaining_bytes() < WORD_LEN {
            return None;
        }
        let read = if let Some(ordinal) = self.members.next() {
            self.input.read_u32().map(|child_id| {
                BankSwitchMembership::Member(BankSwitchMember {
                    group_ordinal: self.current_group,
                    group_value_range: self.current_group_start
                        ..self.current_group_start + WORD_LEN,
                    ordinal,
                    child_id,
                })
            })
        } else {
            let ordinal = self.groups.next()?;
            self.group(ordinal).map(BankSwitchMembership::Group)
        };
        if read.is_err() {
            *self.stopped = true;
        }
        Some(read)
    }
}

impl<R: Read + Seek> BankSwitchGroupRows<'_, R> {
    fn group(&mut self, ordinal: u32) -> Result<BankSwitchGroup, BankReadError> {
        self.current_group = ordinal;
        let value_id = self.input.read_u32()?;
        let start = value_id.range.start;
        self.current_group_start = start;
        let mut group = BankSwitchGroup {
            ordinal,
            value_id,
            member_count: None,
            members_range: None,
            checked_range: start..self.input.offset() + self.input.remaining_bytes(),
            complete: false,
        };
        if self.input.remaining_bytes() < WORD_LEN {
            return Ok(group);
        }
        let count = self.input.read_u32()?;
        let members = self.input.offset();
        let required = u64::from(count.value) * WORD_LEN;
        let checked = required.min(self.input.remaining_bytes());
        self.members = 0..count.value;
        group.member_count = Some(count);
        group.members_range = Some(members..members + checked);
        group.checked_range.end = members + checked;
        group.complete = required == checked;
        Ok(group)
    }
}
