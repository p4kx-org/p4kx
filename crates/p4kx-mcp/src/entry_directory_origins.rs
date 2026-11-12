// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DosCalendarInfo, Entry, EntryMetadataInfo, FieldOrigin, FieldOrigins,
    FieldOriginsError, FieldPath, FieldPathError, MetadataRange, SourceEvidenceError,
    SourceLocation, SourceUse, SourceUseRole, SourceUses,
};
use thiserror::Error;

use crate::content_origin::EntryPageSources;

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

pub(crate) struct EntryDirectoryOrigins {
    facts: EntryPageSources,
    range: MetadataRange,
}

impl EntryDirectoryOrigins {
    pub(crate) fn observe(
        archive: &Archive,
        entry: &Entry,
        uses: SourceUses,
    ) -> Result<Self, EvidenceError> {
        Ok(Self {
            facts: EntryPageSources::observe_into(archive, entry, uses)?,
            range: entry.metadata().record_range(),
        })
    }

    pub(crate) fn write(
        &self,
        fields: &mut FieldOrigins,
        row: &str,
        metadata: &EntryMetadataInfo,
    ) -> Result<Vec<SourceUse>, EvidenceError> {
        let record = self.facts.directory_subresource(
            directory_range("entryDirectoryRecord", "record", self.range),
            SourceUseRole::Payload,
        )?;
        let mut used = self.facts.path_uses.clone();
        used.push(record.clone());
        put(fields, row, self.facts.origin(&used))?;
        put(fields, &format!("{row}/path"), self.facts.path.clone())?;
        put(fields, &format!("{row}/metadata"), self.origin(&[record]))?;
        for (out, source, role) in [
            (
                "compression",
                "raw_compression_method",
                SourceUseRole::Conversion,
            ),
            (
                "encrypted",
                "encryption_declarations",
                SourceUseRole::Conversion,
            ),
            ("compressed_size", "compressed_size", SourceUseRole::Payload),
            ("size", "uncompressed_size", SourceUseRole::Payload),
        ] {
            let source = self.facts.directory_fact(source, role)?;
            put(fields, &format!("{row}/{out}"), self.origin(&[source]))?;
        }
        self.metadata(fields, &format!("{row}/metadata"), metadata)?;
        Ok(used)
    }

    fn metadata(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        metadata: &EntryMetadataInfo,
    ) -> Result<(), EvidenceError> {
        for name in [
            "owner_entry_index",
            "original_directory_ordinal",
            "raw_compression_method",
        ] {
            let usage = self.facts.directory_fact(name, SourceUseRole::Payload)?;
            put(fields, &format!("{path}/{name}"), self.origin(&[usage]))?;
        }
        let format = self
            .facts
            .directory_fact("record_format", SourceUseRole::Selector)?;
        put(
            fields,
            &format!("{path}/record_format"),
            self.origin(std::slice::from_ref(&format)),
        )?;
        let raw_word = if metadata.raw_v2_encryption_word.is_some() {
            self.facts
                .directory_fact("raw_v2_encryption_word", SourceUseRole::Payload)?
        } else {
            format.clone()
        };
        put(
            fields,
            &format!("{path}/raw_v2_encryption_word"),
            self.origin(&[raw_word]),
        )?;
        if metadata.zip_header.is_some() {
            self.zip_header(fields, &format!("{path}/zip_header"))?;
        } else {
            put(
                fields,
                &format!("{path}/zip_header"),
                self.origin(&[format]),
            )?;
        }
        self.timestamp(
            fields,
            &format!("{path}/dos_timestamp"),
            metadata.dos_timestamp.calendar,
        )
    }

    fn zip_header(&self, fields: &mut FieldOrigins, path: &str) -> Result<(), EvidenceError> {
        let uses = [
            "raw_version_made_by",
            "raw_version_needed",
            "general_purpose_flags",
            "disk_start",
            "internal_attributes",
            "external_attributes",
        ]
        .into_iter()
        .map(|name| {
            let usage = self
                .facts
                .directory_fact(&format!("zip_header/{name}"), SourceUseRole::Payload)?;
            put(
                fields,
                &format!("{path}/{name}"),
                self.origin(std::slice::from_ref(&usage)),
            )?;
            Ok(usage)
        })
        .collect::<Result<Vec<_>, EvidenceError>>()?;
        put(fields, path, self.origin(&uses))
    }

    fn timestamp(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        calendar: DosCalendarInfo,
    ) -> Result<(), EvidenceError> {
        let date = self
            .facts
            .directory_fact("dos_timestamp/raw_date", SourceUseRole::Payload)?;
        let time = self
            .facts
            .directory_fact("dos_timestamp/raw_time", SourceUseRole::Payload)?;
        for (name, usage) in [("raw_date", &date), ("raw_time", &time)] {
            put(
                fields,
                &format!("{path}/{name}"),
                self.origin(std::slice::from_ref(usage)),
            )?;
        }
        let both = self.origin(&[date, time]);
        for address in [
            path.to_owned(),
            format!("{path}/calendar"),
            format!("{path}/calendar/state"),
        ] {
            put(fields, &address, both.clone())?;
        }
        if let DosCalendarInfo::Valid { .. } = calendar {
            for name in ["year", "month", "day", "hour", "minute", "second"] {
                put(fields, &format!("{path}/calendar/{name}"), both.clone())?;
            }
        }
        Ok(())
    }

    pub(crate) const fn complete(&self) -> bool {
        self.facts.complete()
    }

    fn origin(&self, used: &[SourceUse]) -> FieldOrigin {
        self.facts.origin(used)
    }
}

pub(crate) fn directory_range(
    namespace: &str,
    field: &str,
    range: MetadataRange,
) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: vec![range.offset(), range.offset() + range.len()],
        field: field.into(),
    }
}

pub(crate) fn put(
    fields: &mut FieldOrigins,
    address: &str,
    origin: FieldOrigin,
) -> Result<(), EvidenceError> {
    Ok(fields.insert(FieldPath::parse(address)?, origin)?)
}
