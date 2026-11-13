// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DdsHeaderField, DdsSourceInfo, FieldOrigin, FieldOrigins, FieldPath, PreviewPngEvidence,
    SourceUseRole, SourceUses,
};

use crate::PackageResult;
use crate::texture_source_metadata::metadata_error;

#[path = "texture_field_words.rs"]
mod words;

pub(super) fn capture(
    dds: &DdsSourceInfo,
    evidence: &PreviewPngEvidence,
    alpha_limits_level: bool,
    uses: &SourceUses,
) -> PackageResult<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    let color = evidence.color.header();
    words::capture(&mut fields, dds, color, uses)?;
    normalized(&mut fields, evidence, uses)?;
    let size = evidence.color.header_size_use().map_err(metadata_error)?;
    insert(
        &mut fields,
        "/dds/header_source_bytes",
        uses.origin(&[size], true),
    )?;
    insert(&mut fields, "/dds/schema_version", uses.origin(&[], true))?;
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

fn normalized(
    fields: &mut FieldOrigins,
    evidence: &PreviewPngEvidence,
    uses: &SourceUses,
) -> PackageResult<()> {
    use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};

    for (path, field) in [
        ("width", Width),
        ("height", Height),
        ("depth", Depth),
        ("mip_count", MipCount),
        ("faces", Faces),
        ("cubemap", Faces),
        ("array_size", ArraySize),
        ("format", Format),
        ("reader_high_range", Format),
        ("block_layout", Format),
    ] {
        let contributors = evidence
            .color
            .header_uses(&[Prefix, field], SourceUseRole::Conversion)
            .map_err(metadata_error)?;
        let origin = uses.origin(&contributors, true);
        let path = format!("/dds/normalized/{path}");
        if path.ends_with("/block_layout") {
            for field in ["block_width", "block_height", "bytes_per_block"] {
                insert(fields, &format!("{path}/{field}"), origin.clone())?;
            }
        }
        insert(fields, &path, origin)?;
    }
    Ok(())
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
