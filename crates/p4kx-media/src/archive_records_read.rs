// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveMetadataVersion, DecimalU64, EntryMetadataInfo, EntryRecordFormat,
    EntryRecordFormatInfo, MetadataRecord,
};

use crate::archive_extras_read;
use crate::archive_metadata_diagnostic;
use crate::archive_raw::ArchiveRawSpan;
use crate::archive_record::ArchiveDirectoryRecord;
use crate::archive_retention::{self, ArchiveRetention};
use crate::{DirectoryContext, MediaError, MediaWriter, Status, Subresource};

pub(crate) const RECORD_NAMESPACE: &str = "archive-directory-record";

pub(crate) fn write(
    context: &mut DirectoryContext<'_, '_, '_>,
    retained: &ArchiveRetention,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut records = match context.view.records() {
        Ok(records) => records,
        Err(cause) => {
            return archive_metadata_diagnostic::structural(
                cause,
                RECORD_NAMESPACE,
                vec![0],
                writer,
            );
        }
    };
    let mut position = 0;
    loop {
        writer.checkpoint()?;
        let record = match records.next_record(context.access) {
            Ok(Some(record)) => record,
            Ok(None) => return Ok(()),
            Err(cause) => {
                return archive_metadata_diagnostic::structural(
                    cause,
                    RECORD_NAMESPACE,
                    vec![0, position],
                    writer,
                );
            }
        };
        let raw = retained.record(position, record.raw(), context.access, writer)?;
        let data = project(position, record, &raw, context)?;
        let issues = if matches!(data.format, EntryRecordFormatInfo::Unknown) {
            vec![archive_metadata_diagnostic::warning(
                "archive.metadata-record-format",
                RECORD_NAMESPACE,
                "The record format has no supported typed interpretation; its raw bytes are retained.",
            )]
        } else {
            Vec::new()
        };
        writer.archive_layout_record(
            &Subresource::new(RECORD_NAMESPACE, vec![position]),
            if issues.is_empty() {
                Status::Complete
            } else {
                Status::Partial
            },
            &data,
            &issues,
        )?;
        archive_extras_read::write(position, record, &raw, context, writer)?;
        position = position
            .checked_add(1)
            .ok_or_else(|| MediaError::Contract("metadata record position overflow".into()))?;
    }
}

fn project(
    position: u64,
    record: MetadataRecord<'_>,
    raw: &ArchiveRawSpan,
    context: &DirectoryContext<'_, '_, '_>,
) -> Result<ArchiveDirectoryRecord, MediaError> {
    let format = match record.format() {
        EntryRecordFormat::ZipCentralDirectory => EntryRecordFormatInfo::ZipCentralDirectory,
        EntryRecordFormat::P4kV2 => EntryRecordFormatInfo::P4kV2,
        _ => EntryRecordFormatInfo::Unknown,
    };
    let metadata = physical_metadata(record.admitted_entry_index(), context)?;
    Ok(ArchiveDirectoryRecord {
        version: ArchiveMetadataVersion::V1,
        position: DecimalU64::new(position),
        original_directory_ordinal: record.original_directory_ordinal().map(DecimalU64::new),
        admitted_entry_index: record
            .admitted_entry_index()
            .map(u64::try_from)
            .transpose()
            .map_err(|_| MediaError::Contract("physical Entry index exceeds source width".into()))?
            .map(DecimalU64::new),
        format,
        metadata,
        raw: raw.clone(),
        name: record
            .zip_name()
            .map(|span| archive_retention::within(raw, span))
            .transpose()?,
        extras: record
            .zip_extra()
            .map(|span| archive_retention::within(raw, span))
            .transpose()?,
        comment: record
            .zip_comment()
            .map(|span| archive_retention::within(raw, span))
            .transpose()?,
        signature: record
            .v2_signature()?
            .map(|span| archive_retention::within(raw, span))
            .transpose()?,
        stored_digest: record
            .v2_stored_digest()?
            .map(|span| archive_retention::within(raw, span))
            .transpose()?,
    })
}

fn physical_metadata(
    index: Option<usize>,
    context: &DirectoryContext<'_, '_, '_>,
) -> Result<Option<EntryMetadataInfo>, MediaError> {
    index
        .map(|index| {
            context
                .entries
                .get(index)
                .filter(|entry| entry.index() == index)
                .map(EntryMetadataInfo::of)
                .ok_or_else(|| {
                    MediaError::Contract(
                        "admitted metadata record has no exact physical Entry".into(),
                    )
                })
        })
        .transpose()
}
