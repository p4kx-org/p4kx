// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveLayout, ArchiveLayoutInfo, ArchiveMetadataStatus};

use crate::{DirectoryContext, MediaError, MediaWriter, Status, Subresource};
use crate::{
    archive_metadata_diagnostic, archive_records_read, archive_sections_read, archive_tables_read,
};

pub(crate) const LAYOUT_NAMESPACE: &str = "archive-layout";

pub(crate) fn write(
    context: &mut DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.checkpoint()?;
    let data = ArchiveLayoutInfo::of(context.view.metadata());
    let issues = if context.view.metadata().status() == ArchiveMetadataStatus::Available
        && !matches!(data.layout, ArchiveLayout::Unknown)
    {
        Vec::new()
    } else {
        vec![archive_metadata_diagnostic::warning(
            "archive.metadata-layout",
            LAYOUT_NAMESPACE,
            "The owning source has malformed or unavailable global layout declarations; lazy row checks remain independent.",
        )]
    };
    writer.record(
        &Subresource::new(LAYOUT_NAMESPACE, Vec::new()),
        if issues.is_empty() {
            Status::Complete
        } else {
            Status::Partial
        },
        &data,
        &issues,
    )?;
    let retained = archive_sections_read::write(context, writer)?;
    archive_records_read::write(context, &retained, writer)?;
    archive_tables_read::write(context, &retained, writer)?;
    writer.checkpoint()
}
