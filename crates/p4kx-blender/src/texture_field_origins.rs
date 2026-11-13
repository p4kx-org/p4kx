// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DdsHeaderField, DdsSourceInfo, FieldOrigin, FieldOrigins, FieldPath, PreviewPngEvidence,
    SourceUseRole, SourceUses,
};

use crate::PackageResult;
use crate::texture_source_metadata::metadata_error;
use p4kx_extraction::texture_preview;
use p4kx_texture::{DdsAlphaTransform, DdsPngEncoding};

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
    encoding: DdsPngEncoding,
    alpha_transform: DdsAlphaTransform,
    uses: &SourceUses,
) -> PackageResult<FieldOrigin> {
    let mut contributors = Vec::new();
    evidence
        .append_pixel_uses(&mut contributors)
        .map_err(metadata_error)?;
    contributors.extend(
        evidence
            .color
            .header()
            .output_transfer_uses(encoding)
            .map_err(metadata_error)?,
    );
    if alpha_transform != DdsAlphaTransform::Preserve {
        contributors.extend(alpha_uses(evidence)?);
    }
    Ok(uses.origin(&contributors, evidence.is_complete()))
}

pub(super) fn output_fields(
    fields: &mut FieldOrigins,
    evidence: &PreviewPngEvidence,
    encoding: DdsPngEncoding,
    uses: &SourceUses,
) -> PackageResult<()> {
    let header = evidence.color.header();
    let transfer = header
        .output_transfer_uses(encoding)
        .map_err(metadata_error)?;
    let transform = header
        .header_uses(
            &[DdsHeaderField::Prefix, DdsHeaderField::Format],
            SourceUseRole::Conversion,
        )
        .map_err(metadata_error)?;
    let complete = evidence.color.is_complete();
    insert(fields, "/output_transfer", uses.origin(&transfer, complete))?;
    insert(fields, "/png_transform", uses.origin(&transform, complete))
}

pub(super) fn alpha_transform(
    fields: &mut FieldOrigins,
    evidence: &PreviewPngEvidence,
    uses: &SourceUses,
) -> PackageResult<()> {
    let mut contributors = alpha_uses(evidence)?;
    contributors.extend(
        evidence
            .color
            .header()
            .header_uses(
                &[DdsHeaderField::DxgiTransfer, DdsHeaderField::CryTransfer],
                SourceUseRole::Conversion,
            )
            .map_err(metadata_error)?,
    );
    insert(
        fields,
        "/alpha_transform",
        uses.origin(&contributors, evidence.color.is_complete()),
    )
}

fn alpha_uses(evidence: &PreviewPngEvidence) -> PackageResult<Vec<p4kx_extraction::SourceUse>> {
    evidence
        .color
        .header()
        .header_uses(
            &[DdsHeaderField::Prefix, DdsHeaderField::AlphaAssociation],
            SourceUseRole::Conversion,
        )
        .map_err(metadata_error)
}

pub(super) fn insert(
    fields: &mut FieldOrigins,
    path: &str,
    origin: FieldOrigin,
) -> PackageResult<()> {
    let path = FieldPath::parse(path).map_err(metadata_error)?;
    fields.insert(path, origin).map_err(metadata_error)
}
