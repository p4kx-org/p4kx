// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveMetadataVersion, ArchiveRegion, DecimalU64, MetadataFreeStatus, MetadataProgressBound,
    MetadataReservedWord, MetadataTableRow, MetadataTableValue,
};

use crate::archive_metadata_diagnostic;
use crate::archive_raw::ArchiveRawSpan;
use crate::archive_retention::ArchiveRetention;
use crate::archive_table::{
    ArchiveFreeRange, ArchiveFreeState, ArchiveInstallation, ArchiveProgressBound,
    ArchiveReservedState,
};
use crate::{DirectoryContext, MediaError, MediaWriter, Status, Subresource};

pub(crate) const INSTALL_NAMESPACE: &str = "archive-installation";
pub(crate) const FREE_NAMESPACE: &str = "archive-free-range";

#[derive(Clone, Copy)]
enum Table {
    Installation,
    Free,
}

pub(crate) fn write(
    context: &mut DirectoryContext<'_, '_, '_>,
    retained: &ArchiveRetention,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for table in [Table::Installation, Table::Free] {
        write_table(table, context, retained, writer)?;
    }
    Ok(())
}

fn write_table(
    table: Table,
    context: &mut DirectoryContext<'_, '_, '_>,
    retained: &ArchiveRetention,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (namespace, operation) = match table {
        Table::Installation => (INSTALL_NAMESPACE, 2),
        Table::Free => (FREE_NAMESPACE, 3),
    };
    let rows = match table {
        Table::Installation => context.view.installation_records(),
        Table::Free => context.view.free_records(),
    };
    let mut rows = match rows {
        Ok(Some(rows)) => rows,
        Ok(None) => return Ok(()),
        Err(cause) => {
            return archive_metadata_diagnostic::structural(
                cause,
                namespace,
                vec![operation],
                writer,
            );
        }
    };
    loop {
        writer.checkpoint()?;
        let row = match rows.next_record(context.access) {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(()),
            Err(cause) => {
                return archive_metadata_diagnostic::structural(
                    cause,
                    namespace,
                    vec![operation],
                    writer,
                );
            }
        };
        let raw = retained.existing(row.raw())?;
        match row.value() {
            MetadataTableValue::Installation(value) => installation(row, raw, value, writer)?,
            MetadataTableValue::FreeRange(value) => free(row, raw, value, writer)?,
            _ => {
                return Err(MediaError::Contract(
                    "unknown metadata table-row type".into(),
                ));
            }
        }
    }
}

fn installation(
    row: MetadataTableRow<'_>,
    raw: ArchiveRawSpan,
    value: p4kx_extraction::MetadataInstallValue,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let bound = match value.bound() {
        MetadataProgressBound::Unavailable => ArchiveProgressBound::Unavailable,
        MetadataProgressBound::WithinStoredSize => ArchiveProgressBound::WithinStoredSize,
        MetadataProgressBound::ExceedsStoredSize => ArchiveProgressBound::ExceedsStoredSize,
        _ => {
            return Err(MediaError::Contract(
                "unknown metadata progress bound".into(),
            ));
        }
    };
    let reserved_state = match value.reserved_status() {
        MetadataReservedWord::NotPresent => ArchiveReservedState::NotPresent,
        MetadataReservedWord::Zero => ArchiveReservedState::Zero,
        MetadataReservedWord::Nonzero => ArchiveReservedState::Nonzero,
        _ => {
            return Err(MediaError::Contract(
                "unknown metadata reserved-word status".into(),
            ));
        }
    };
    let valid = value.bound() != MetadataProgressBound::ExceedsStoredSize
        && value.reserved_status() != MetadataReservedWord::Nonzero;
    let data = ArchiveInstallation {
        version: ArchiveMetadataVersion::V1,
        ordinal: DecimalU64::new(row.ordinal()),
        raw,
        written: DecimalU64::new(value.written()),
        reserved: value.reserved(),
        reserved_state,
        bound,
    };
    emit(INSTALL_NAMESPACE, row.ordinal(), valid, &data, writer)
}

fn free(
    row: MetadataTableRow<'_>,
    raw: ArchiveRawSpan,
    value: p4kx_extraction::MetadataFreeValue,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let state = match value.status() {
        MetadataFreeStatus::WithinSource => ArchiveFreeState::WithinSource,
        MetadataFreeStatus::ZeroSize => ArchiveFreeState::ZeroSize,
        MetadataFreeStatus::OutsideSource => ArchiveFreeState::OutsideSource,
        MetadataFreeStatus::Overflow => ArchiveFreeState::Overflow,
        _ => {
            return Err(MediaError::Contract(
                "unknown metadata free-range status".into(),
            ));
        }
    };
    let data = ArchiveFreeRange {
        version: ArchiveMetadataVersion::V1,
        ordinal: DecimalU64::new(row.ordinal()),
        raw,
        extent: ArchiveRegion::of(value.extent()),
        state,
    };
    emit(
        FREE_NAMESPACE,
        row.ordinal(),
        matches!(
            value.status(),
            MetadataFreeStatus::WithinSource | MetadataFreeStatus::ZeroSize
        ),
        &data,
        writer,
    )
}

fn emit<T: serde::Serialize>(
    namespace: &str,
    ordinal: u64,
    valid: bool,
    data: &T,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let issues = if valid {
        Vec::new()
    } else {
        vec![archive_metadata_diagnostic::warning(
            "archive.metadata-table-value",
            namespace,
            "The original table row contains an invalid progress, reserved-word or free-range declaration; its raw bytes are retained.",
        )]
    };
    writer.archive_layout_record(
        &Subresource::new(namespace, vec![ordinal]),
        if valid {
            Status::Complete
        } else {
            Status::Partial
        },
        data,
        &issues,
    )
}
