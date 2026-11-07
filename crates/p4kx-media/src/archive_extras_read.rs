// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveMetadataVersion, DecimalU64, MetadataExtra, MetadataExtraStatus, MetadataRecord,
};

use crate::archive_metadata_diagnostic;
use crate::archive_raw::ArchiveRawSpan;
use crate::archive_record::{ArchiveExtraData, ArchiveExtraState};
use crate::archive_retention;
use crate::{DirectoryContext, MediaError, MediaWriter, Status, Subresource};

pub(crate) const EXTRA_NAMESPACE: &str = "archive-extra";

pub(crate) fn write<'source>(
    position: u64,
    record: MetadataRecord<'source>,
    parent: &ArchiveRawSpan,
    context: &mut DirectoryContext<'source, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(mut extras) = record.extras() else {
        return Ok(());
    };
    loop {
        writer.checkpoint()?;
        let extra = match extras.next_field(context.access) {
            Ok(Some(extra)) => extra,
            Ok(None) => return Ok(()),
            Err(cause) => {
                return archive_metadata_diagnostic::structural(
                    cause,
                    EXTRA_NAMESPACE,
                    vec![1, position],
                    writer,
                );
            }
        };
        let complete = extra.status() == MetadataExtraStatus::Complete;
        let data = project(position, extra, parent)?;
        let issues = if complete {
            Vec::new()
        } else {
            vec![archive_metadata_diagnostic::warning(
                "archive.metadata-extra-framing",
                EXTRA_NAMESPACE,
                "The original extra field is malformed; all available field or tail bytes are retained.",
            )]
        };
        writer.record(
            &Subresource::new(EXTRA_NAMESPACE, vec![position, extra.ordinal()]),
            if complete {
                Status::Complete
            } else {
                Status::Partial
            },
            &data,
            &issues,
        )?;
        if !complete {
            return Ok(());
        }
    }
}

fn project(
    position: u64,
    extra: MetadataExtra<'_>,
    parent: &ArchiveRawSpan,
) -> Result<ArchiveExtraData, MediaError> {
    let state = match extra.status() {
        MetadataExtraStatus::Complete => ArchiveExtraState::Complete,
        MetadataExtraStatus::ShortHeader => ArchiveExtraState::ShortHeader,
        MetadataExtraStatus::LengthBeforeHeader => ArchiveExtraState::LengthBeforeHeader,
        MetadataExtraStatus::TruncatedBody => ArchiveExtraState::TruncatedBody,
        _ => return Err(MediaError::Contract("unknown extra framing state".into())),
    };
    Ok(ArchiveExtraData {
        version: ArchiveMetadataVersion::V1,
        record_position: DecimalU64::new(position),
        ordinal: DecimalU64::new(extra.ordinal()),
        id: extra.id(),
        declared_total_length: extra.declared_total_len(),
        state,
        raw: archive_retention::within(parent, extra.raw())?,
        body: extra
            .body()
            .map(|span| archive_retention::within(parent, span))
            .transpose()?,
    })
}
