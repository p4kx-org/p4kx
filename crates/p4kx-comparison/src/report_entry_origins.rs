// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_extraction::{
    ArchiveLayoutOrigins, EntryDirectoryOrigins, FieldOrigins, SourceEvidenceError, SourceUses,
};

#[path = "report_entry_decision.rs"]
mod decision;

use crate::report::{EntryChangeReport, EntryState, IncomparableEntryReport};
use crate::report_entry_aggregates::AggregateOrigins;
use crate::{Comparison, DirectoryCounts};

use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};

pub(super) fn capture(
    comparison: &Comparison<'_>,
    changes: &[EntryChangeReport<'_>],
    incomparable: &[IncomparableEntryReport<'_>],
    directories: &[DirectoryCounts<'_>],
) -> Result<ReportOriginFields, ReportOriginsError> {
    let uses = SourceUses::with_compact_use_refs();
    let old = comparison.old_side().owner();
    let new = comparison.new_side().owner();
    let mut fields = FieldOrigins::new();
    let mut aggregates = AggregateOrigins::new(directories);
    let mut collisions = decision::CollisionOrigins::default();
    for (index, (change, row)) in comparison.entry_changes().iter().zip(changes).enumerate() {
        let path = format!("/entries/changes/{index}");
        let before = observe_side(
            old,
            change.old_entry().zip(row.old.as_ref()),
            &uses,
            &mut fields,
            &format!("{path}/old"),
        )?;
        let after = observe_side(
            new,
            change.new_entry().zip(row.new.as_ref()),
            &uses,
            &mut fields,
            &format!("{path}/new"),
        )?;
        let selected = after
            .as_ref()
            .or(before.as_ref())
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        put(
            &mut fields,
            &format!("{path}/path"),
            selected.path_origin().clone(),
        )?;
        let origin = decision::capture(
            change.decision(),
            before.as_ref(),
            after.as_ref(),
            comparison,
            &uses,
            &mut collisions,
        )?;
        if row.pairing_ambiguous {
            put(
                &mut fields,
                &format!("{path}/pairing_ambiguous"),
                origin.clone(),
            )?;
        }
        aggregates.observe(change, selected, &origin, &uses)?;
        put(&mut fields, &format!("{path}/change"), origin)?;
    }
    aggregates.write(&uses, &mut fields)?;
    observe_incomparable(
        comparison,
        incomparable,
        &uses,
        &mut fields,
        &mut collisions,
    )?;
    ReportOriginFields::finish(fields, &uses)
}

fn observe_incomparable(
    comparison: &Comparison<'_>,
    reports: &[IncomparableEntryReport<'_>],
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    collisions: &mut decision::CollisionOrigins,
) -> Result<(), ReportOriginsError> {
    if reports.is_empty() {
        return Ok(());
    }
    let old = comparison.old_side().owner();
    let new = comparison.new_side().owner();
    let layouts = [
        ArchiveLayoutOrigins::layout_origin(old, uses)?,
        ArchiveLayoutOrigins::layout_origin(new, uses)?,
    ];
    for (index, (entry, row)) in comparison
        .incomparable_entries()
        .iter()
        .zip(reports)
        .enumerate()
    {
        let path = format!("/entries/incomparable/{index}");
        let before = observe_side(
            old,
            Some((entry.old_entry(), &row.old)),
            uses,
            fields,
            &format!("{path}/old"),
        )?;
        let after = observe_side(
            new,
            Some((entry.new_entry(), &row.new)),
            uses,
            fields,
            &format!("{path}/new"),
        )?;
        let after = after.as_ref().ok_or(SourceEvidenceError::ForeignEvidence)?;
        put(fields, &format!("{path}/path"), after.path_origin().clone())?;
        let reason = decision::capture(
            entry.decision(),
            before.as_ref(),
            Some(after),
            comparison,
            uses,
            collisions,
        )?;
        let reason =
            uses.origin_with_origins(layouts.iter().chain(std::iter::once(&reason)), true)?;
        if row.pairing_ambiguous {
            put(fields, &format!("{path}/pairing_ambiguous"), reason.clone())?;
        }
        put(fields, &format!("{path}/reason"), reason)?;
    }
    Ok(())
}

fn observe_side(
    owner: &Archive,
    candidate: Option<(&Entry, &EntryState)>,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    side: &str,
) -> Result<Option<EntryDirectoryOrigins>, ReportOriginsError> {
    let Some((entry, state)) = candidate else {
        return Ok(None);
    };
    let observed = EntryDirectoryOrigins::observe(owner, entry, uses.clone())?;
    put(fields, &format!("{side}/size"), observed.size_origin()?)?;
    if let Some(checksum) = &state.checksum {
        let field = checksum.field_name();
        put(
            fields,
            &format!("{side}/{field}"),
            observed.checksum_origin()?,
        )?;
    }
    Ok(Some(observed))
}
