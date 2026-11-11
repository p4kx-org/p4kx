// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DdsHeaderField, FieldOrigin, ObservedPreviewPng, PreviewPngEvidence, PreviewTextureEvidence,
    SourceEvidenceError, SourceUse, SourceUseRole, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;

use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
use SourceUseRole::{Conversion, Payload, Selector};

const TOPOLOGY: &[DdsHeaderField] = &[Faces, ArraySize, Depth];
const PIXELS: &[DdsHeaderField] = &[
    Prefix, Format, Width, Height, MipCount, Faces, ArraySize, Depth,
];

/// Each key addresses its value in the complete MCP tool result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImageFieldOrigins {
    #[serde(rename = "/structuredContent/archive")]
    archive: FieldOrigin,
    #[serde(rename = "/structuredContent/path")]
    path: FieldOrigin,
    #[serde(rename = "/structuredContent/format")]
    format: FieldOrigin,
    #[serde(rename = "/structuredContent/texture_width")]
    texture_width: FieldOrigin,
    #[serde(rename = "/structuredContent/texture_height")]
    texture_height: FieldOrigin,
    #[serde(rename = "/structuredContent/mip_count")]
    mip_count: FieldOrigin,
    #[serde(rename = "/structuredContent/mip")]
    mip: FieldOrigin,
    #[serde(rename = "/structuredContent/width")]
    width: FieldOrigin,
    #[serde(rename = "/structuredContent/height")]
    height: FieldOrigin,
    #[serde(rename = "/structuredContent/cubemap")]
    cubemap: FieldOrigin,
    #[serde(
        rename = "/structuredContent/face",
        skip_serializing_if = "Option::is_none"
    )]
    face: Option<FieldOrigin>,
    #[serde(rename = "/structuredContent/array_size")]
    array_size: FieldOrigin,
    #[serde(rename = "/structuredContent/depth")]
    depth: FieldOrigin,
    #[serde(rename = "/structuredContent/alpha_tail")]
    alpha_tail: FieldOrigin,
    #[serde(rename = "/structuredContent/mime_type")]
    mime_type: FieldOrigin,
    #[serde(rename = "/structuredContent/png_bytes")]
    png_bytes: FieldOrigin,
    #[serde(rename = "/content/1/data")]
    png: FieldOrigin,
}

impl ImageFieldOrigins {
    pub(crate) fn capture(
        shown: &ObservedPreviewPng<'_>,
        uses: &SourceUses,
        fit: bool,
        one_face: bool,
    ) -> Result<Self, SourceEvidenceError> {
        let evidence = shown.evidence(uses)?;
        let color = &evidence.color;
        let raw = |field| {
            color
                .header_uses(&[field], Payload)
                .map(|retained| uses.origin(&retained, true))
        };

        let face_selector = if one_face {
            color.header_uses(&[Faces], Selector)?
        } else {
            Vec::new()
        };
        let mip_selector = mip_uses(color, fit, one_face)?;
        let layout = if one_face {
            Vec::new()
        } else {
            color.header_uses(TOPOLOGY, Conversion)?
        };
        let dimensions = |field| -> Result<FieldOrigin, SourceEvidenceError> {
            let mut derived = color.header_uses(&[field], Conversion)?;
            derived.extend_from_slice(&layout);
            derived.extend_from_slice(&mip_selector);
            derived.extend_from_slice(&face_selector);
            Ok(uses.origin(&derived, true))
        };

        let (png, alpha_tail) = rendered_origins(&evidence, uses, &mip_selector, &face_selector)?;

        Ok(Self {
            archive: uses.origin(&[], true),
            path: color.path_origin(uses)?,
            format: raw(Format)?,
            texture_width: raw(Width)?,
            texture_height: raw(Height)?,
            mip_count: raw(MipCount)?,
            mip: uses.origin(&mip_selector, true),
            width: dimensions(Width)?,
            height: dimensions(Height)?,
            cubemap: raw(Faces)?,
            face: one_face.then(|| uses.origin(&face_selector, true)),
            array_size: raw(ArraySize)?,
            depth: raw(Depth)?,
            alpha_tail,
            mime_type: uses.origin(&[], true),
            png_bytes: png.clone(),
            png,
        })
    }
}

fn mip_uses(
    color: &PreviewTextureEvidence,
    fit: bool,
    one_face: bool,
) -> Result<Vec<SourceUse>, SourceEvidenceError> {
    let mut mip = color.header_uses(&[MipCount], Selector)?;
    if fit {
        mip.extend(color.header_uses(&[Width, Height], Selector)?);
        if !one_face {
            mip.extend(color.header_uses(TOPOLOGY, Selector)?);
        }
    }
    Ok(mip)
}

fn rendered_origins(
    evidence: &PreviewPngEvidence,
    uses: &SourceUses,
    mip_selector: &[SourceUse],
    face_selector: &[SourceUse],
) -> Result<(FieldOrigin, FieldOrigin), SourceEvidenceError> {
    let mut alpha_lookup = vec![evidence.color.name_use(Selector)?];
    let mut png = mip_selector.to_vec();
    png.extend_from_slice(face_selector);
    append_image_inputs(&evidence.color, &mut png)?;
    if let Some(alpha) = &evidence.alpha {
        alpha_lookup.push(alpha.name_use(Selector)?);
        append_image_inputs(alpha, &mut png)?;
    }
    png.extend_from_slice(&alpha_lookup);
    Ok((uses.origin(&png, true), uses.origin(&alpha_lookup, true)))
}

fn append_image_inputs(
    texture: &PreviewTextureEvidence,
    png: &mut Vec<SourceUse>,
) -> Result<(), SourceEvidenceError> {
    png.extend(texture.header_uses(PIXELS, Conversion)?);
    png.push(texture.header_size_use()?);
    png.push(texture.mip().clone());
    Ok(())
}
