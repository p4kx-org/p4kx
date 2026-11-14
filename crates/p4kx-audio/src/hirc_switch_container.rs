// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

pub(crate) const WORD_LEN: u64 = size_of::<u32>() as u64;
pub(crate) const CONFIGURATION_LEN: u64 = 3 * WORD_LEN + 1;

/// Original decision fields following a version-154 Switch container node base.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSwitchContainerFields {
    /// Start of the original decision continuation.
    pub start: u64,
    /// Original group type code.
    pub group_type: Option<BankHircField<u8>>,
    /// Original switch or state group ID.
    pub group_id: Option<BankHircField<u32>>,
    /// Original default value ID, including zero.
    pub default_value_id: Option<BankHircField<u32>>,
    /// Original continuous-validation byte.
    pub continuous_validation: Option<BankHircField<u8>>,
    /// Original membership group count and checked prefix when reached.
    pub groups: Option<BankSwitchExtent>,
    /// Original child configuration count and checked prefix when reached.
    pub configurations: Option<BankSwitchExtent>,
}

impl BankSwitchContainerFields {
    pub(crate) const fn new(start: u64) -> Self {
        Self {
            start,
            group_type: None,
            group_id: None,
            default_value_id: None,
            continuous_validation: None,
            groups: None,
            configurations: None,
        }
    }

    /// Returns the original decision extent when both declared lists fit.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        let configurations = self.configurations.as_ref()?;
        configurations
            .is_complete()
            .then_some(self.start..configurations.checked_range.end)
    }
}

/// An original Switch table count and checked physical prefix.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSwitchExtent {
    /// Original authored count.
    pub declared_count: BankHircField<u32>,
    /// Original table bytes, including any incomplete final field.
    pub checked_range: Range<u64>,
    complete: bool,
}

impl BankSwitchExtent {
    /// Returns whether all declared rows fit their original body.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

pub(crate) fn header<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankSwitchContainerFields,
) -> Result<(), F::Error> {
    observed.group_type = Some(input.read_u8()?);
    observed.group_id = Some(input.read_u32()?);
    observed.default_value_id = Some(input.read_u32()?);
    observed.continuous_validation = Some(input.read_u8()?);
    Ok(())
}

pub(crate) fn tables<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankSwitchContainerFields,
) -> Result<(), F::Error> {
    let declared_count = input.read_u32()?;
    let start = input.offset();
    let groups = scan_groups(input, declared_count.value);
    let complete = groups.is_ok();
    let end = if complete {
        input.offset()
    } else {
        input.offset() + input.remaining_bytes()
    };
    observed.groups = Some(BankSwitchExtent {
        declared_count,
        checked_range: start..end,
        complete,
    });
    groups?;

    let declared_count = input.read_u32()?;
    let start = input.offset();
    let required = u64::from(declared_count.value) * CONFIGURATION_LEN;
    let available = input.remaining_bytes();
    observed.configurations = Some(BankSwitchExtent {
        declared_count,
        checked_range: start..start + required.min(available),
        complete: required <= available,
    });
    input.skip_bytes(required)
}

fn scan_groups<F: ObjectFields>(input: &mut F, count: u32) -> Result<(), F::Error> {
    for _ in 0..count {
        input.skip_bytes(WORD_LEN)?;
        let members = input.read_u32()?.value;
        input.skip_bytes(u64::from(members) * WORD_LEN)?;
    }
    Ok(())
}
