// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_archive::Archive;
use p4kx_extraction::{
    EntryDirectoryFact, EntryDirectoryOrigins, EntryDirectoryOriginsError, FieldOrigin,
    SourceDetails, SourceEvidenceError, SourceUse, SourceUseRole, SourceUses,
};

use crate::Comparison;
use crate::entry_change::EntryDecision;

pub(super) fn capture(
    decision: EntryDecision,
    old: Option<&EntryDirectoryOrigins>,
    new: Option<&EntryDirectoryOrigins>,
    comparison: &Comparison<'_>,
    uses: &SourceUses,
    collisions: &mut CollisionOrigins,
) -> Result<FieldOrigin, EntryDirectoryOriginsError> {
    if let EntryDecision::Collision { evidence, .. } = decision {
        let origin = collisions.get(evidence, comparison, uses)?;
        return if old.is_some() && new.is_some() {
            Ok(origin.matched.clone())
        } else {
            Ok(origin.unmatched.clone())
        };
    }
    match (old, new) {
        (Some(old), Some(new)) => matched(decision, [old, new], uses),
        (Some(old), None) => unmatched(decision, old, comparison.new_side().owner(), uses),
        (None, Some(new)) => unmatched(decision, new, comparison.old_side().owner(), uses),
        (None, None) => Err(SourceEvidenceError::ForeignEvidence.into()),
    }
}

fn matched(
    decision: EntryDecision,
    entries: [&EntryDirectoryOrigins; 2],
    uses: &SourceUses,
) -> Result<FieldOrigin, EntryDirectoryOriginsError> {
    let compare_checksums = match decision {
        EntryDecision::SizeDiffers => false,
        EntryDecision::ChecksumDiffers => true,
        _ => return Err(SourceEvidenceError::ForeignEvidence.into()),
    };
    let mut observed = Vec::new();
    for entry in entries {
        observed.push(entry.use_fact(EntryDirectoryFact::Path, SourceUseRole::Selector)?);
        observed.push(entry.use_fact(EntryDirectoryFact::Size, SourceUseRole::Conversion)?);
        if compare_checksums {
            observed.push(entry.use_fact(EntryDirectoryFact::Checksum, SourceUseRole::Conversion)?);
        }
    }
    Ok(uses.origin(
        &observed,
        entries.into_iter().all(EntryDirectoryOrigins::complete),
    ))
}

fn unmatched(
    decision: EntryDecision,
    selected: &EntryDirectoryOrigins,
    opposite: &Archive,
    uses: &SourceUses,
) -> Result<FieldOrigin, EntryDirectoryOriginsError> {
    match decision {
        EntryDecision::OrderedAgainst(position) => {
            let head = opposite
                .entries()
                .get(position)
                .ok_or(SourceEvidenceError::ForeignEvidence)?;
            let head = EntryDirectoryOrigins::observe_with_details(
                opposite,
                head,
                uses.clone(),
                SourceDetails::CaptureLimited,
            )?;
            let selected_path =
                selected.use_fact(EntryDirectoryFact::Path, SourceUseRole::Selector)?;
            let opposing_path = head.use_fact(EntryDirectoryFact::Path, SourceUseRole::Selector)?;
            Ok(uses.origin(&[selected_path, opposing_path], false))
        }
        EntryDecision::OppositeExhausted => {
            let selected =
                selected.use_fact(EntryDirectoryFact::Position, SourceUseRole::Selector)?;
            Ok(uses.origin(&[selected], false))
        }
        _ => Err(SourceEvidenceError::ForeignEvidence.into()),
    }
}

#[derive(Default)]
pub(super) struct CollisionOrigins {
    groups: HashMap<usize, CollisionOrigin>,
}

struct CollisionOrigin {
    matched: FieldOrigin,
    unmatched: FieldOrigin,
}

impl CollisionOrigins {
    fn get(
        &mut self,
        index: usize,
        comparison: &Comparison<'_>,
        uses: &SourceUses,
    ) -> Result<&CollisionOrigin, EntryDirectoryOriginsError> {
        match self.groups.entry(index) {
            Entry::Occupied(slot) => Ok(slot.into_mut()),
            Entry::Vacant(slot) => {
                Ok(slot.insert(CollisionOrigin::capture(index, comparison, uses)?))
            }
        }
    }
}

impl CollisionOrigin {
    fn capture(
        index: usize,
        comparison: &Comparison<'_>,
        uses: &SourceUses,
    ) -> Result<Self, EntryDirectoryOriginsError> {
        let group = comparison
            .entry_collisions
            .get(index)
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        let mut operands = Vec::new();
        let mut complete = true;
        for (archive, positions) in [
            (comparison.old_side().owner(), &group.old),
            (comparison.new_side().owner(), &group.new),
        ] {
            for &position in positions {
                complete &= collision_operand(archive, position, uses, &mut operands)?;
            }
        }
        Ok(Self {
            matched: uses.origin(&operands, complete),
            unmatched: uses.origin(&operands, false),
        })
    }
}

fn collision_operand(
    archive: &Archive,
    position: usize,
    uses: &SourceUses,
    operands: &mut Vec<SourceUse>,
) -> Result<bool, EntryDirectoryOriginsError> {
    let entry = archive
        .entries()
        .get(position)
        .ok_or(SourceEvidenceError::ForeignEvidence)?;
    let entry = EntryDirectoryOrigins::observe_with_details(
        archive,
        entry,
        uses.clone(),
        SourceDetails::CaptureLimited,
    )?;
    for fact in [EntryDirectoryFact::Position, EntryDirectoryFact::Path] {
        operands.push(entry.use_fact(fact, SourceUseRole::Selector)?);
    }
    for fact in [EntryDirectoryFact::Size, EntryDirectoryFact::Checksum] {
        operands.push(entry.use_fact(fact, SourceUseRole::Conversion)?);
    }
    Ok(entry.complete())
}
