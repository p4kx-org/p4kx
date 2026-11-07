// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveMetadataVersion, ArchiveRegion, MetadataReadError, MetadataRegionStatus, MetadataSection,
};

use crate::archive_metadata_diagnostic;
use crate::archive_retention::ArchiveRetention;
use crate::archive_section::{
    ArchiveSection, ArchiveSectionData, ArchiveSectionPresence, SECTIONS,
};
use crate::{DirectoryContext, MediaError, MediaIssue, MediaWriter, Status, Subresource};

pub(crate) const SECTION_NAMESPACE: &str = "archive-section";

struct SectionProjection {
    data: ArchiveSectionData,
    issues: Vec<MediaIssue>,
}

pub(crate) fn write(
    context: &mut DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<ArchiveRetention, MediaError> {
    let mut retained = ArchiveRetention::new();
    for (index, (section, reader_section)) in SECTIONS.iter().copied().enumerate() {
        writer.checkpoint()?;
        let index = u64::try_from(index)
            .map_err(|_| MediaError::Contract("section index exceeds source width".into()))?;
        let projection = read(context, section, reader_section, &mut retained, writer)?;
        writer.record(
            &Subresource::new(SECTION_NAMESPACE, vec![index]),
            if projection.issues.is_empty() {
                Status::Complete
            } else {
                Status::Partial
            },
            &projection.data,
            &projection.issues,
        )?;
    }
    Ok(retained)
}

fn read(
    context: &mut DirectoryContext<'_, '_, '_>,
    section: ArchiveSection,
    reader: MetadataSection,
    retained: &mut ArchiveRetention,
    writer: &mut MediaWriter<'_>,
) -> Result<SectionProjection, MediaError> {
    let mut projection = SectionProjection {
        data: ArchiveSectionData {
            version: ArchiveMetadataVersion::V1,
            section,
            presence: ArchiveSectionPresence::Absent,
            region: None,
            raw: None,
        },
        issues: Vec::new(),
    };
    let problem = match context.view.region(reader) {
        Ok(Some(region)) => {
            projection.data.presence = ArchiveSectionPresence::Declared;
            projection.data.region = Some(ArchiveRegion::of(region));
            if let Some(available) = context.view.available_span(reader)? {
                projection.data.raw =
                    Some(retained.section(section, available.span(), context.access, writer)?);
            }
            (region.status() != MetadataRegionStatus::Complete).then_some((
                "archive.metadata-extent",
                "The complete declared metadata extent is not available.",
            ))
        }
        Ok(None) => None,
        Err(MetadataReadError::UnavailableLayout) => {
            projection.data.presence = ArchiveSectionPresence::UnavailableLayout;
            Some((
                "archive.metadata-layout-unavailable",
                "Recovery did not establish this global metadata section.",
            ))
        }
        Err(MetadataReadError::MalformedSection) => {
            projection.data.presence = ArchiveSectionPresence::InvalidDeclaration;
            Some((
                "archive.metadata-declaration",
                "The retained declaration does not establish this metadata location.",
            ))
        }
        Err(cause) => return Err(cause.into()),
    };
    if let Some((code, detail)) = problem {
        projection.issues.push(archive_metadata_diagnostic::warning(
            code,
            SECTION_NAMESPACE,
            detail,
        ));
    }
    Ok(projection)
}
