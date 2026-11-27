// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_extraction::{
    ArchiveLayoutOrigins, EntryDirectoryOrigins, FieldOrigin, FieldOrigins, SourceEvidenceError,
    SourceUses,
};

#[path = "report_entry_decision.rs"]
mod decision;

use crate::entry_change::EntryDecision;
use crate::report::{EntryChangeReport, EntryState, IncomparableEntryReport};
use crate::report_entry_aggregates::AggregateOrigins;
use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};
use crate::{CancelToken, Comparison, DirectoryCounts, IncomparableEntry};

pub(super) fn capture(
    comparison: &Comparison<'_>,
    changes: &[EntryChangeReport<'_>],
    incomparable: &[IncomparableEntryReport<'_>],
    directories: &[DirectoryCounts<'_>],
) -> Result<ReportOriginFields, ReportOriginsError> {
    let mut capture = Capture::new(comparison);
    let mut fields = FieldOrigins::new();
    let mut aggregates = AggregateOrigins::new(directories, comparison.filter().is_some());
    for (index, (change, row)) in comparison.entry_changes().iter().zip(changes).enumerate() {
        let path = format!("/entries/changes/{index}");
        let pair = capture.observe(change.old_entry(), change.new_entry())?;
        pair.write(&path, row.old.as_ref(), row.new.as_ref(), &mut fields)?;
        let origin = capture.decision(change.decision(), &pair)?;
        if row.pairing_ambiguous {
            put(
                &mut fields,
                &format!("{path}/pairing_ambiguous"),
                origin.clone(),
            )?;
        }
        aggregates.observe(change, pair.selected()?, &origin, &capture.uses)?;
        put(&mut fields, &format!("{path}/change"), origin)?;
    }
    aggregates.write(&capture.uses, &mut fields)?;
    capture.write_incomparables(incomparable, &mut fields)?;
    ReportOriginFields::finish(fields, &capture.uses)
}

pub(crate) fn capture_summary(
    comparison: &Comparison<'_>,
    cancel: &CancelToken,
) -> Result<Option<ReportOriginFields>, ReportOriginsError> {
    if cancel.is_cancelled() {
        return Ok(None);
    }
    let mut capture = Capture::new(comparison);
    let directories = comparison.directory_counts();
    let mut aggregates = AggregateOrigins::new(&directories, comparison.filter().is_some());
    for change in comparison.entry_changes() {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let pair = capture.observe(change.old_entry(), change.new_entry())?;
        let origin = capture.decision(change.decision(), &pair)?;
        aggregates.observe(change, pair.selected()?, &origin, &capture.uses)?;
    }
    let Some(incomparables) = capture.incomparable_count(cancel)? else {
        return Ok(None);
    };
    let mut fields = FieldOrigins::new();
    aggregates.write(&capture.uses, &mut fields)?;
    put(&mut fields, "/entries/incomparableCount", incomparables)?;
    if cancel.is_cancelled() {
        return Ok(None);
    }
    let evidence = ReportOriginFields::finish(fields, &capture.uses)?;
    Ok((!cancel.is_cancelled()).then_some(evidence))
}

struct Capture<'comparison, 'archive> {
    comparison: &'comparison Comparison<'archive>,
    uses: SourceUses,
    collisions: decision::CollisionOrigins,
}

impl<'comparison, 'archive> Capture<'comparison, 'archive> {
    fn new(comparison: &'comparison Comparison<'archive>) -> Self {
        Self {
            comparison,
            uses: SourceUses::with_compact_use_refs(),
            collisions: decision::CollisionOrigins::default(),
        }
    }

    fn observe(
        &self,
        old: Option<&Entry>,
        new: Option<&Entry>,
    ) -> Result<EntryPair, ReportOriginsError> {
        Ok(EntryPair {
            old: observe_side(self.comparison.old_side().owner(), old, &self.uses)?,
            new: observe_side(self.comparison.new_side().owner(), new, &self.uses)?,
        })
    }

    fn decision(
        &mut self,
        kind: EntryDecision,
        pair: &EntryPair,
    ) -> Result<FieldOrigin, ReportOriginsError> {
        Ok(decision::capture(
            kind,
            pair.old.as_ref(),
            pair.new.as_ref(),
            self.comparison,
            &self.uses,
            &mut self.collisions,
        )?)
    }

    fn layouts(&self) -> Result<[FieldOrigin; 2], ReportOriginsError> {
        Ok([
            ArchiveLayoutOrigins::layout_origin(self.comparison.old_side().owner(), &self.uses)?,
            ArchiveLayoutOrigins::layout_origin(self.comparison.new_side().owner(), &self.uses)?,
        ])
    }

    fn incomparable_reason(
        &mut self,
        entry: &IncomparableEntry<'_>,
        pair: &EntryPair,
        layouts: &[FieldOrigin; 2],
    ) -> Result<FieldOrigin, ReportOriginsError> {
        let reason = self.decision(entry.decision(), pair)?;
        Ok(self
            .uses
            .origin_with_origins(layouts.iter().chain(std::iter::once(&reason)), true)?)
    }

    fn incomparable_count(
        &mut self,
        cancel: &CancelToken,
    ) -> Result<Option<FieldOrigin>, ReportOriginsError> {
        let entries = self.comparison.incomparable_entries();
        if entries.is_empty() {
            return Ok(Some(self.uses.origin(&[], false)));
        }
        let layouts = self.layouts()?;
        let mut reasons = Vec::new();
        for entry in entries {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            let pair = self.observe(Some(entry.old_entry()), Some(entry.new_entry()))?;
            reasons.push(self.incomparable_reason(entry, &pair, &layouts)?);
        }
        Ok(Some(self.uses.origin_with_origins(&reasons, true)?))
    }

    fn write_incomparables(
        &mut self,
        reports: &[IncomparableEntryReport<'_>],
        fields: &mut FieldOrigins,
    ) -> Result<(), ReportOriginsError> {
        if reports.is_empty() {
            return Ok(());
        }
        let layouts = self.layouts()?;
        for (index, (entry, row)) in self
            .comparison
            .incomparable_entries()
            .iter()
            .zip(reports)
            .enumerate()
        {
            let path = format!("/entries/incomparable/{index}");
            let pair = self.observe(Some(entry.old_entry()), Some(entry.new_entry()))?;
            pair.write(&path, Some(&row.old), Some(&row.new), fields)?;
            let reason = self.incomparable_reason(entry, &pair, &layouts)?;
            if row.pairing_ambiguous {
                put(fields, &format!("{path}/pairing_ambiguous"), reason.clone())?;
            }
            put(fields, &format!("{path}/reason"), reason)?;
        }
        Ok(())
    }
}

struct EntryPair {
    old: Option<EntryDirectoryOrigins>,
    new: Option<EntryDirectoryOrigins>,
}

impl EntryPair {
    fn selected(&self) -> Result<&EntryDirectoryOrigins, SourceEvidenceError> {
        self.new
            .as_ref()
            .or(self.old.as_ref())
            .ok_or(SourceEvidenceError::ForeignEvidence)
    }

    fn write(
        &self,
        path: &str,
        old: Option<&EntryState>,
        new: Option<&EntryState>,
        fields: &mut FieldOrigins,
    ) -> Result<(), ReportOriginsError> {
        write_side(self.old.as_ref().zip(old), fields, &format!("{path}/old"))?;
        write_side(self.new.as_ref().zip(new), fields, &format!("{path}/new"))?;
        put(
            fields,
            &format!("{path}/path"),
            self.selected()?.path_origin().clone(),
        )
    }
}

fn observe_side(
    owner: &Archive,
    entry: Option<&Entry>,
    uses: &SourceUses,
) -> Result<Option<EntryDirectoryOrigins>, ReportOriginsError> {
    Ok(entry
        .map(|entry| EntryDirectoryOrigins::observe(owner, entry, uses.clone()))
        .transpose()?)
}

fn write_side(
    observed: Option<(&EntryDirectoryOrigins, &EntryState)>,
    fields: &mut FieldOrigins,
    side: &str,
) -> Result<(), ReportOriginsError> {
    if let Some((observed, state)) = observed {
        put(fields, &format!("{side}/size"), observed.size_origin()?)?;
        if let Some(checksum) = &state.checksum {
            let field = checksum.field_name();
            put(
                fields,
                &format!("{side}/{field}"),
                observed.checksum_origin()?,
            )?;
        }
    }
    Ok(())
}
