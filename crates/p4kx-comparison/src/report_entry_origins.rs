// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_extraction::{EntryDirectoryOrigins, FieldOrigins, SourceUses};

#[path = "report_entry_decision.rs"]
mod decision;

use crate::Comparison;
use crate::report::{ChecksumReport, EntryChangeReport};

use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};

pub(super) fn capture(
    comparison: &Comparison<'_>,
    changes: &[EntryChangeReport<'_>],
) -> Result<ReportOriginFields, ReportOriginsError> {
    let uses = SourceUses::with_compact_use_refs();
    let old = comparison.old_side().owner();
    let new = comparison.new_side().owner();
    let mut fields = FieldOrigins::new();
    for (index, (change, row)) in comparison.entry_changes().iter().zip(changes).enumerate() {
        let path = format!("/entries/changes/{index}");
        let before = change
            .old_entry()
            .zip(row.old.as_ref())
            .map(|(entry, state)| {
                observe(
                    old,
                    entry,
                    state.checksum.as_ref(),
                    &uses,
                    &mut fields,
                    &format!("{path}/old"),
                )
            })
            .transpose()?;
        let after = change
            .new_entry()
            .zip(row.new.as_ref())
            .map(|(entry, state)| {
                observe(
                    new,
                    entry,
                    state.checksum.as_ref(),
                    &uses,
                    &mut fields,
                    &format!("{path}/new"),
                )
            })
            .transpose()?;
        if let Some(selected) = after.as_ref().or(before.as_ref()) {
            put(
                &mut fields,
                &format!("{path}/path"),
                selected.path_origin().clone(),
            )?;
        }
        let origin = decision::capture(
            change.decision(),
            before.as_ref(),
            after.as_ref(),
            comparison,
            &uses,
        )?;
        put(&mut fields, &format!("{path}/change"), origin)?;
    }
    ReportOriginFields::finish(fields, &uses)
}

fn observe(
    owner: &Archive,
    entry: &Entry,
    checksum: Option<&ChecksumReport>,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    side: &str,
) -> Result<EntryDirectoryOrigins, ReportOriginsError> {
    let observed = EntryDirectoryOrigins::observe(owner, entry, uses.clone())?;
    put(fields, &format!("{side}/size"), observed.size_origin()?)?;
    if let Some(checksum) = checksum {
        let field = checksum.field_name();
        put(
            fields,
            &format!("{side}/{field}"),
            observed.checksum_origin()?,
        )?;
    }
    Ok(observed)
}
