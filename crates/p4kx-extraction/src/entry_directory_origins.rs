// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry, MetadataRange};
use thiserror::Error;

#[path = "entry_directory_source.rs"]
mod source;

use crate::{
    DosCalendarInfo, EntryMetadataInfo, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, SourceEvidenceError, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};
use source::EntryDirectorySource;
pub use source::directory_path_uses;

/// An Entry directory field could not be bound to its output path.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EntryDirectoryOriginsError {
    /// The shared source registry could not bind an original use.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    /// The output path is not a valid JSON pointer.
    #[error(transparent)]
    Path(#[from] FieldPathError),
    /// The field already has an original source binding.
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

/// Field-level origins for an admitted physical Entry directory row.
pub struct EntryDirectoryOrigins {
    facts: EntryDirectorySource,
    range: MetadataRange,
}

impl EntryDirectoryOrigins {
    /// Binds an Entry to its actual owning Archive and source registry.
    ///
    /// # Errors
    /// Returns an error when the source registry cannot bind the original Entry.
    pub fn observe(
        archive: &Archive,
        entry: &Entry,
        uses: SourceUses,
    ) -> Result<Self, EntryDirectoryOriginsError> {
        Ok(Self {
            facts: EntryDirectorySource::observe(archive, entry, uses)?,
            range: entry.metadata().record_range(),
        })
    }

    /// Records the physical Entry fields at the given JSON pointer.
    ///
    /// # Errors
    /// Returns an error for invalid paths, duplicate fields or a failed source binding.
    pub fn write(
        &self,
        fields: &mut FieldOrigins,
        row: &str,
        metadata: &EntryMetadataInfo,
    ) -> Result<Vec<SourceUse>, EntryDirectoryOriginsError> {
        let record = self.record_use()?;
        let mut used = self.facts.path_uses.clone();
        used.push(record.clone());
        put(fields, row, self.origin(&used))?;
        self.write_header(fields, row, metadata, &record)?;
        self.write_sizes(fields, row, &[])?;
        Ok(used)
    }

    /// Records the physical header fields and the original contributors of a split texture.
    ///
    /// # Errors
    /// Returns an error for invalid paths, duplicate fields or a failed source binding.
    pub fn write_group(
        &self,
        fields: &mut FieldOrigins,
        row: &str,
        metadata: &EntryMetadataInfo,
        parts: &[Self],
    ) -> Result<(), EntryDirectoryOriginsError> {
        let record = self.record_use()?;
        self.write_header(fields, row, metadata, &record)?;
        let mut part_paths = Vec::new();
        let mut row_uses = self.facts.path_uses.clone();
        row_uses.push(record);
        let mut complete = self.complete();
        for (ordinal, part) in parts.iter().enumerate() {
            put(
                fields,
                &format!("{row}/parts/{ordinal}"),
                part.path_origin().clone(),
            )?;
            part_paths.extend_from_slice(&part.facts.path_uses);
            row_uses.push(part.record_use()?);
            complete &= part.complete();
        }
        row_uses.extend(part_paths.iter().cloned());
        put(
            fields,
            &format!("{row}/parts"),
            self.facts.origin_when(&part_paths, complete),
        )?;
        put(fields, row, self.facts.origin_when(&row_uses, complete))?;
        self.write_sizes(fields, row, parts)
    }

    fn record_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.facts.directory_subresource(
            directory_range("entryDirectoryRecord", "record", self.range),
            SourceUseRole::Payload,
        )
    }

    fn write_header(
        &self,
        fields: &mut FieldOrigins,
        row: &str,
        metadata: &EntryMetadataInfo,
        record: &SourceUse,
    ) -> Result<(), EntryDirectoryOriginsError> {
        put(fields, &format!("{row}/path"), self.facts.path.clone())?;
        put(
            fields,
            &format!("{row}/metadata"),
            self.origin(std::slice::from_ref(record)),
        )?;
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
        ] {
            let source = self.facts.directory_fact(source, role)?;
            put(fields, &format!("{row}/{out}"), self.origin(&[source]))?;
        }
        self.metadata(fields, &format!("{row}/metadata"), metadata)
    }

    fn write_sizes(
        &self,
        fields: &mut FieldOrigins,
        row: &str,
        parts: &[Self],
    ) -> Result<(), EntryDirectoryOriginsError> {
        let complete = self.complete() && parts.iter().all(Self::complete);
        for (out, source) in [
            ("compressed_size", "compressed_size"),
            ("size", "uncompressed_size"),
        ] {
            let contributors = std::iter::once(self)
                .chain(parts)
                .map(|entry| entry.facts.directory_fact(source, SourceUseRole::Payload))
                .collect::<Result<Vec<_>, _>>()?;
            put(
                fields,
                &format!("{row}/{out}"),
                self.facts.origin_when(&contributors, complete),
            )?;
        }
        Ok(())
    }

    fn metadata(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        metadata: &EntryMetadataInfo,
    ) -> Result<(), EntryDirectoryOriginsError> {
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

    fn zip_header(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
    ) -> Result<(), EntryDirectoryOriginsError> {
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
        .collect::<Result<Vec<_>, EntryDirectoryOriginsError>>()?;
        put(fields, path, self.origin(&uses))
    }

    fn timestamp(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        calendar: DosCalendarInfo,
    ) -> Result<(), EntryDirectoryOriginsError> {
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

    /// Returns the source binding of the Entry path.
    #[must_use]
    pub const fn path_origin(&self) -> &FieldOrigin {
        &self.facts.path
    }

    /// Returns whether the observed Entry and all container ancestors were bound.
    #[must_use]
    pub const fn complete(&self) -> bool {
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
) -> Result<(), EntryDirectoryOriginsError> {
    Ok(fields.insert(FieldPath::parse(address)?, origin)?)
}
