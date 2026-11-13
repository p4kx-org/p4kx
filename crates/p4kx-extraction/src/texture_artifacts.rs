// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{
    DdsAlphaAssociation, DdsAlphaTransform, DdsHeaderField, DdsPngEncoding, DdsPngTransform,
    DdsTransfer,
};
use serde::Serialize;

use crate::texture_preview::DdsInfoOriginError;
use crate::texture_preview_png::input::PreviewInputs;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, OriginArtifactDigest, SourceInventory, SourceUseRole,
    SourceUses, TextureImageFace,
};

/// The converted images published from one texture and their shared source inventory.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TextureArtifacts {
    /// Images committed by this extraction in DDS storage order.
    pub images: Vec<TextureImageArtifact>,
    /// The effective transfer declared in the actual colour DDS header.
    pub source_transfer: DdsTransfer,
    /// The transfer of the encoded samples common to this image set.
    pub output_transfer: DdsTransfer,
    /// Channel processing for PNG output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_transform: Option<DdsPngTransform>,
    /// Original DDS alpha declaration from the same selected read.
    pub alpha_association: DdsAlphaAssociation,
    /// RGB unassociation applied to the emitted PNG samples.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_transform: Option<DdsAlphaTransform>,
    /// Exact source contributions of the transfer and transform values.
    pub field_origins: FieldOrigins,
    /// The inventory in which each image's source use references resolve.
    pub provenance: SourceInventory,
}

/// The byte binding and contributors of one converted image published by bulk extraction.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TextureImageArtifact {
    /// The image path relative to the extraction target, with forward slash separators.
    pub path: String,
    /// The measured byte length and SHA-256 of the committed image.
    pub digest: OriginArtifactDigest,
    /// The actual selected mip level, with zero denoting the largest.
    pub level: u32,
    /// The zero-based source array layer.
    pub layer: u32,
    /// The source face.
    pub face: TextureImageFace,
    /// The zero-based volume slice at this level.
    pub slice: u32,
    /// The header fields and mip payloads that supplied this image's pixels.
    #[serde(flatten)]
    pub origin: FieldOrigin,
}

pub(crate) struct TextureImageTransfer {
    source: DdsTransfer,
    output: DdsTransfer,
    png: Option<DdsPngEncoding>,
    alpha_association: DdsAlphaAssociation,
    alpha_transform: Option<DdsAlphaTransform>,
    origins: FieldOrigins,
}

impl TextureImageTransfer {
    pub(crate) fn capture<Spans>(
        inputs: &PreviewInputs<'_, Spans>,
        source: DdsTransfer,
        png: Option<DdsPngEncoding>,
        transform: DdsAlphaTransform,
        uses: &SourceUses,
    ) -> Result<Self, DdsInfoOriginError> {
        use DdsHeaderField::{CryTransfer, DxgiTransfer, Format, Prefix};

        let (header, complete) = inputs.color_header_evidence(uses)?;
        let declared = header.header_uses(
            &[Prefix, DxgiTransfer, CryTransfer],
            SourceUseRole::Conversion,
        )?;
        let mut origins = FieldOrigins::new();
        origins.insert(
            FieldPath::parse("/source_transfer")?,
            uses.origin(&declared, complete),
        )?;
        let format = header.header_uses(&[Prefix, Format], SourceUseRole::Conversion)?;
        let output = if let Some(png) = png {
            origins.insert(
                FieldPath::parse("/png_transform")?,
                uses.origin(&format, complete),
            )?;
            header.output_transfer_uses(png)?
        } else {
            format
        };
        origins.insert(
            FieldPath::parse("/output_transfer")?,
            uses.origin(&output, complete),
        )?;
        let association = header.alpha_association_uses()?;
        origins.insert(
            FieldPath::parse("/alpha_association")?,
            uses.origin(&association, complete),
        )?;
        let alpha_transform = (transform != DdsAlphaTransform::Preserve).then_some(transform);
        if alpha_transform.is_some() {
            let mut transform_uses = association;
            transform_uses.extend(output);
            origins.insert(
                FieldPath::parse("/alpha_transform")?,
                uses.origin(&transform_uses, complete),
            )?;
        }
        Ok(Self {
            source,
            output: png.map_or(DdsTransfer::Linear, |encoding| encoding.transfer),
            png,
            alpha_association: inputs.source_alpha().0,
            alpha_transform,
            origins,
        })
    }

    pub(crate) fn finish(
        self,
        images: Vec<TextureImageArtifact>,
        provenance: SourceInventory,
    ) -> TextureArtifacts {
        TextureArtifacts {
            images,
            source_transfer: self.source,
            output_transfer: self.output,
            png_transform: self.png.map(|encoding| encoding.transform),
            alpha_association: self.alpha_association,
            alpha_transform: self.alpha_transform,
            field_origins: self.origins,
            provenance,
        }
    }
}
