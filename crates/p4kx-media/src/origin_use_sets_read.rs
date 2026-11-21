// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::MediaError;
use crate::origin_contract::use_sets::{SetId, SetRow};
use crate::origin_inventory_wire::{self, Encryption};
use crate::origin_line_reader::OriginLines;
use crate::origin_read_policy::InventoryBudget;

#[derive(Clone, Copy)]
pub(super) struct Evidence {
    pub(super) encryption: Encryption,
    pub(super) opaque: bool,
}

impl Evidence {
    pub(super) const fn empty() -> Self {
        Self {
            encryption: Encryption::Unencrypted,
            opaque: false,
        }
    }

    pub(super) const fn include(&mut self, other: Self) {
        self.encryption = self.encryption.combine(other.encryption);
        self.opaque |= other.opaque;
    }

    pub(super) fn validate(self, marker: Option<bool>, complete: bool) -> Result<(), MediaError> {
        if self.opaque && complete {
            return Err(invalid(
                "opaque contributing use requires incomplete field coverage",
            ));
        }
        let expected = match (self.encryption, complete) {
            (Encryption::Encrypted, _) => Some(true),
            (Encryption::Unencrypted, true) => Some(false),
            (Encryption::Unknown, _) | (Encryption::Unencrypted, false) => None,
        };
        if marker != expected {
            return Err(invalid(
                "field marker disagrees with its descriptive source-use evidence",
            ));
        }
        Ok(())
    }
}

pub(super) struct ReadSet {
    pub(super) members: Vec<String>,
    pub(super) evidence: Evidence,
}

impl ReadSet {
    pub(super) fn contains(&self, reference: &str) -> bool {
        self.members
            .binary_search_by(|member| member.as_str().cmp(reference))
            .is_ok()
    }
}

pub(super) fn read(
    mut lines: OriginLines<'_, '_>,
    budget: &mut InventoryBudget,
    evaluate: impl FnMut(&str) -> Result<Evidence, MediaError>,
) -> Result<Vec<ReadSet>, MediaError> {
    let sets = read_frames(
        || lines.next(origin_inventory_wire::seed::<SetRow<String>>()),
        budget,
        evaluate,
    )?;
    lines.finish()?;
    Ok(sets)
}

fn read_frames(
    mut next: impl FnMut() -> Result<Option<SetRow<String>>, MediaError>,
    budget: &mut InventoryBudget,
    mut evaluate: impl FnMut(&str) -> Result<Evidence, MediaError>,
) -> Result<Vec<ReadSet>, MediaError> {
    let mut sets = Vec::new();
    let mut all_members = 0_u64;
    loop {
        match next()? {
            Some(SetRow::Set { id, members })
                if id.0.get() == sets.len() as u64 && members.get() > 0 =>
            {
                budget.charge(size_of::<ReadSet>())?;
                sets.push(read_members(
                    &mut next,
                    id,
                    members.get(),
                    budget,
                    &mut evaluate,
                )?);
                all_members = all_members
                    .checked_add(members.get())
                    .ok_or_else(|| invalid("origin use-set member counter overflow"))?;
            }
            Some(SetRow::Eof {
                sets: count,
                members,
            }) if count.get() == sets.len() as u64 && members.get() == all_members => {
                if next()?.is_some() {
                    return Err(invalid("origin use-set stream has trailing rows"));
                }
                return Ok(sets);
            }
            _ => {
                return Err(invalid(
                    "origin use-set header, sequence or EOF is incomplete",
                ));
            }
        }
    }
}

fn read_members(
    next: &mut impl FnMut() -> Result<Option<SetRow<String>>, MediaError>,
    id: SetId,
    count: u64,
    budget: &mut InventoryBudget,
    evaluate: &mut impl FnMut(&str) -> Result<Evidence, MediaError>,
) -> Result<ReadSet, MediaError> {
    let mut set = ReadSet {
        members: Vec::new(),
        evidence: Evidence::empty(),
    };
    for _ in 0..count {
        let Some(SetRow::Member { reference }) = next()? else {
            return Err(invalid(
                "origin use-set member count disagrees with its body",
            ));
        };
        if set
            .members
            .last()
            .is_some_and(|previous| previous >= &reference)
        {
            return Err(invalid("origin use-set members are not sorted and unique"));
        }
        budget.charge(reference.len().saturating_add(size_of::<String>()))?;
        set.evidence.include(evaluate(&reference)?);
        set.members.push(reference);
    }
    match next()? {
        Some(SetRow::End { id: end }) if end == id => Ok(set),
        _ => Err(invalid(
            "origin use-set closing row is missing or mismatched",
        )),
    }
}

fn invalid(message: &str) -> MediaError {
    MediaError::Destination(message.to_owned())
}

#[cfg(test)]
#[path = "origin_use_sets_read_tests.rs"]
mod tests;
