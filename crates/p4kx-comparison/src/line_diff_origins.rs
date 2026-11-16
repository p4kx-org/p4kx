// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    EntryDirectoryFact, FieldOrigin, FieldOrigins, SourceEvidenceError, SourceUseRole, SourceUses,
};

use crate::decode::DecodeOutcome;
use crate::line_diff_hunks;
use crate::line_diff_side::SideOrigins;
use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};
use crate::{DiffBody, LineDiff};

pub(crate) fn capture(diff: &LineDiff<'_>) -> Result<ReportOriginFields, ReportOriginsError> {
    let uses = SourceUses::with_compact_use_refs();
    let [old, new] = diff.original_inputs();
    let old = SideOrigins::capture(old, &uses)?;
    let new = SideOrigins::capture(new, &uses)?;
    let mut fields = FieldOrigins::new();
    old.write(&mut fields, "/old", &uses)?;
    new.write(&mut fields, "/new", &uses)?;
    let path_origin = match (&old.directory, &new.directory) {
        (_, Some(directory)) => {
            uses.origin_with_origins([&new.lookup, directory.path_origin()], true)?
        }
        (Some(directory), None) => {
            uses.origin_with_origins([&old.lookup, &new.lookup, directory.path_origin()], true)?
        }
        (None, None) => return Err(SourceEvidenceError::ForeignEvidence.into()),
    };
    put(&mut fields, "/path", path_origin)?;
    let change = change(&old, &new, &uses)?;
    let content = content(&old, &new, &uses)?;
    let differs = if diff.kind().is_some() {
        change.clone()
    } else {
        uses.origin_with_origins([&change, &content], true)?
    };
    put(&mut fields, "/change", change)?;
    put(&mut fields, "/differs", differs)?;
    put(&mut fields, "/content", content.clone())?;
    put(&mut fields, "/hunks", content.clone())?;
    if let DiffBody::Text(text) = diff.body() {
        line_diff_hunks::write(&mut fields, text, [&old, &new], &content, &uses)?;
    }
    ReportOriginFields::finish(fields, &uses)
}

fn change(
    old: &SideOrigins<'_>,
    new: &SideOrigins<'_>,
    uses: &SourceUses,
) -> Result<FieldOrigin, ReportOriginsError> {
    let Some((old_entry, new_entry)) = old.entry.zip(new.entry) else {
        return Ok(uses.origin_with_origins([&old.lookup, &new.lookup], true)?);
    };
    let (old_dir, new_dir) = old
        .directory
        .as_ref()
        .zip(new.directory.as_ref())
        .ok_or(SourceEvidenceError::ForeignEvidence)?;
    let compare_checksum = old_entry.uncompressed_size() == new_entry.uncompressed_size();
    let mut underlying = Vec::new();
    for directory in [old_dir, new_dir] {
        underlying.push(directory.use_fact(EntryDirectoryFact::Size, SourceUseRole::Conversion)?);
        if compare_checksum {
            underlying
                .push(directory.use_fact(EntryDirectoryFact::Checksum, SourceUseRole::Conversion)?);
        }
    }
    let operands = uses.origin(&underlying, old_dir.complete() && new_dir.complete());
    Ok(uses.origin_with_origins([&old.lookup, &new.lookup, &operands], true)?)
}

fn content(
    old: &SideOrigins<'_>,
    new: &SideOrigins<'_>,
    uses: &SourceUses,
) -> Result<FieldOrigin, ReportOriginsError> {
    if matches!(old.outcome, Some(DecodeOutcome::TooLarge(_))) {
        Ok(old.body.clone())
    } else if matches!(new.outcome, Some(DecodeOutcome::TooLarge(_))) {
        Ok(new.body.clone())
    } else {
        Ok(uses.origin_with_origins([&old.body, &new.body], true)?)
    }
}
