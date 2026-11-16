// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    EntryDirectoryFact, EntryDirectoryOrigins, FieldOrigin, FieldOrigins, InputScope,
    SourceCoordinateOmission, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUseRole, SourceUses,
};

use crate::decode::{DecodeOutcome, LimitSource};
use crate::line_diff::LineInput;
use crate::report_origins::{ReportOriginsError, put};
use crate::{Encoding, Entry};

pub(crate) struct SideOrigins<'archive> {
    pub(crate) entry: Option<&'archive Entry>,
    pub(crate) directory: Option<EntryDirectoryOrigins>,
    pub(crate) lookup: FieldOrigin,
    pub(crate) body: FieldOrigin,
    pub(crate) outcome: Option<DecodeOutcome>,
    payload: Option<SourceScope>,
}

impl<'archive> SideOrigins<'archive> {
    pub(crate) fn capture(
        input: LineInput<'archive>,
        uses: &SourceUses,
    ) -> Result<Self, ReportOriginsError> {
        let archive = input.archive;
        let entry = input.entry;
        let prefix = entry.map_or_else(|| archive.entries().len(), |entry| entry.index() + 1);
        let observed = uses.observe_root_directory_path_prefix(archive, prefix)?;
        let directory = entry
            .map(|entry| EntryDirectoryOrigins::observe(archive, entry, uses.clone()))
            .transpose()?;
        let mut lookup: Vec<_> = observed.iter().cloned().collect();
        if let Some(directory) = &directory {
            lookup.push(directory.use_fact(EntryDirectoryFact::Path, SourceUseRole::Selector)?);
        }
        let complete = observed.is_some()
            && directory
                .as_ref()
                .is_none_or(EntryDirectoryOrigins::complete);
        let lookup = uses.origin(&lookup, complete);

        let payload = match (entry, input.outcome) {
            (Some(entry), Some(outcome))
                if outcome != DecodeOutcome::TooLarge(LimitSource::DeclaredSize) =>
            {
                let original = archive
                    .observed_entry(entry)
                    .map_or(SourceInput::Unknown, SourceInput::ArchiveView);
                Some(uses.observe(original, InputScope::Payload)?)
            }
            _ => None,
        };
        let body = side_body(entry, directory.as_ref(), payload.as_ref(), &lookup, uses)?;
        Ok(Self {
            entry,
            directory,
            lookup,
            body,
            outcome: input.outcome,
            payload,
        })
    }

    pub(crate) fn write(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        uses: &SourceUses,
    ) -> Result<(), ReportOriginsError> {
        put(fields, path, self.lookup.clone())?;
        if let Some((entry, directory)) = self.entry.zip(self.directory.as_ref()) {
            let size = directory.size_origin()?;
            put(
                fields,
                &format!("{path}/size"),
                uses.origin_with_origins([&self.lookup, &size], true)?,
            )?;
            let checksum = directory.checksum_origin()?;
            put(
                fields,
                &format!("{path}/{}", entry.checksum().name()),
                uses.origin_with_origins([&self.lookup, &checksum], true)?,
            )?;
            put(fields, &format!("{path}/encoding"), self.body.clone())?;
        }
        Ok(())
    }

    pub(crate) fn line(
        &self,
        range: Option<Range<u64>>,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, ReportOriginsError> {
        let payload = self
            .payload
            .as_ref()
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        let role = if matches!(self.outcome, Some(DecodeOutcome::Text(Encoding::Text))) {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Conversion
        };
        let fact = match range {
            Some(range) => payload.at(entry_range(range, "bytes"), role)?,
            None => payload.at_opaque(role, SourceCoordinateOmission::Unavailable)?,
        };
        let mut facts = vec![fact];
        if matches!(
            self.outcome,
            Some(DecodeOutcome::Text(Encoding::Utf16Le | Encoding::Utf16Be))
        ) {
            facts.push(payload.at(entry_range(0..2, "byteOrderMark"), SourceUseRole::Selector)?);
        }
        let body = uses.origin(
            &facts,
            self.directory
                .as_ref()
                .is_some_and(EntryDirectoryOrigins::complete),
        );
        uses.origin_with_origins([&self.lookup, &body], true)
            .map_err(Into::into)
    }
}

fn side_body(
    entry: Option<&Entry>,
    directory: Option<&EntryDirectoryOrigins>,
    payload: Option<&SourceScope>,
    lookup: &FieldOrigin,
    uses: &SourceUses,
) -> Result<FieldOrigin, ReportOriginsError> {
    let Some((entry, directory)) = entry.zip(directory) else {
        return Ok(lookup.clone());
    };
    let input = match payload {
        Some(scope) => scope.at(
            entry_range(0..entry.uncompressed_size(), "bytes"),
            SourceUseRole::Conversion,
        )?,
        None => directory.use_fact(EntryDirectoryFact::Size, SourceUseRole::Conversion)?,
    };
    let body = uses.origin(&[input], directory.complete());
    uses.origin_with_origins([lookup, &body], true)
        .map_err(Into::into)
}

fn entry_range(range: Range<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "entryDecodedBytes".into(),
        indices: vec![range.start, range.end],
        field: field.into(),
    }
}
