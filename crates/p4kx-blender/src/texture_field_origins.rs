// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DdsHeaderField, DdsSourceInfo, FieldOrigin, FieldOrigins, FieldPath, PreviewPngEvidence,
    SourceUseRole, SourceUses,
};

use crate::PackageResult;
use crate::texture_source_metadata::metadata_error;
use p4kx_extraction::texture_preview;

pub(super) fn capture(
    dds: &DdsSourceInfo,
    evidence: &PreviewPngEvidence,
    alpha_limits_level: bool,
    uses: &SourceUses,
) -> PackageResult<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    let color = evidence.color.header();
    let base = FieldPath::parse("/dds").map_err(metadata_error)?;
    texture_preview::dds_info_origins(&mut fields, &base, dds, color, uses)
        .map_err(metadata_error)?;
    let mut contributors = color
        .header_uses(&[DdsHeaderField::MipCount], SourceUseRole::Selector)
        .map_err(metadata_error)?;
    if alpha_limits_level && let Some(alpha) = &evidence.alpha {
        contributors.extend(
            alpha
                .header_uses(&[DdsHeaderField::MipCount], SourceUseRole::Selector)
                .map_err(metadata_error)?,
        );
    }
    insert(
        &mut fields,
        "/mip_level",
        uses.origin(&contributors, evidence.is_complete()),
    )?;
    Ok(fields)
}

pub(super) fn image_origin(
    evidence: &PreviewPngEvidence,
    uses: &SourceUses,
) -> PackageResult<FieldOrigin> {
    let mut contributors = Vec::new();
    evidence
        .append_pixel_uses(&mut contributors)
        .map_err(metadata_error)?;
    Ok(uses.origin(&contributors, evidence.is_complete()))
}

pub(super) fn insert(
    fields: &mut FieldOrigins,
    path: &str,
    origin: FieldOrigin,
) -> PackageResult<()> {
    let path = FieldPath::parse(path).map_err(metadata_error)?;
    fields.insert(path, origin).map_err(metadata_error)
}
