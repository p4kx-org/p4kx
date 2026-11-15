// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;
use p4kx_extraction::{
    EntryDirectoryFact, EntryDirectoryOrigins, EntryDirectoryOriginsError, FieldOrigin,
    SourceDetails, SourceEvidenceError, SourceUseRole, SourceUses,
};

use crate::Comparison;
use crate::entry_change::EntryDecision;

pub(super) fn capture(
    decision: EntryDecision,
    old: Option<&EntryDirectoryOrigins>,
    new: Option<&EntryDirectoryOrigins>,
    comparison: &Comparison<'_>,
    uses: &SourceUses,
) -> Result<FieldOrigin, EntryDirectoryOriginsError> {
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
