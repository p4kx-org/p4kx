// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::{ObservedPreviewImage, PreviewHeaderEvidence};
use p4kx_extraction::{
    DdsHeaderField, FieldOrigin, SourceEvidenceError, SourceInventory, SourceUseRole, SourceUses,
    Texture,
};

use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Width};
use SourceUseRole::{Conversion, Payload, Selector};

/// Origins of the facts and derived geometry of an opened texture header.
#[derive(Debug)]
#[non_exhaustive]
pub struct ImageHeaderProvenance {
    /// Stored header path in its owning Archive directory.
    pub path: FieldOrigin,
    /// Normalized pixel format.
    pub format: FieldOrigin,
    /// Width of mip zero.
    pub width: FieldOrigin,
    /// Height of mip zero.
    pub height: FieldOrigin,
    /// Number of mips.
    pub mip_count: FieldOrigin,
    /// Cubemap face count or its absence.
    pub cubemap_faces: FieldOrigin,
    /// Array or volume layer count or its absence.
    pub layers: FieldOrigin,
    /// Whether the format can be decoded.
    pub decodable: FieldOrigin,
    /// Inputs common to each mip selector label.
    pub mip_label: FieldOrigin,
    /// Inputs common to the laid-out width of each selectable mip.
    pub image_width: FieldOrigin,
    /// Inputs common to the laid-out height of each selectable mip.
    pub image_height: FieldOrigin,
    inventory: SourceInventory,
}

impl ImageHeaderProvenance {
    /// Returns the physical sources and use coordinates for the header facts.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(super) fn capture(texture: &Texture<'_>) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let header = texture.preview_header_evidence(&uses)?;
        let origin = |fields: &[DdsHeaderField], role| origin(&header, &uses, fields, role);
        let [image_width, image_height] = geometry(&header, &uses)?;
        Ok(Self {
            path: uses.origin(&[header.name_use(Payload)?], true),
            format: origin(&[Format], Payload)?,
            width: origin(&[Width], Payload)?,
            height: origin(&[Height], Payload)?,
            mip_count: origin(&[MipCount], Payload)?,
            cubemap_faces: origin(&[Faces], Conversion)?,
            layers: origin(&[ArraySize, Depth], Conversion)?,
            decodable: origin(&[Format], Conversion)?,
            mip_label: origin(&[Width, Height, MipCount], Conversion)?,
            image_width,
            image_height,
            inventory: uses.freeze()?,
        })
    }
}

/// Origins of the selected mip and the pixels uploaded to the preview.
#[derive(Debug)]
#[non_exhaustive]
pub struct ImageMipProvenance {
    /// Header inputs used to admit the requested mip.
    pub level: FieldOrigin,
    /// Width of the decoded and laid-out mip.
    pub width: FieldOrigin,
    /// Height of the decoded and laid-out mip.
    pub height: FieldOrigin,
    /// Original color and alpha inputs that contributed to the rendered pixels.
    pub pixels: FieldOrigin,
    inventory: SourceInventory,
}

impl ImageMipProvenance {
    /// Returns the physical sources and use coordinates for this mip.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(super) fn capture(image: &ObservedPreviewImage<'_>) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let evidence = image.evidence(&uses)?;
        let header = evidence.color.header();
        let [width, height] = geometry(header, &uses)?;
        let selectors = header.header_uses(
            &[Format, Width, Height, MipCount, Faces, ArraySize, Depth],
            Selector,
        )?;
        let level = uses.origin(&selectors, true);
        let mut pixels = selectors;
        pixels.extend(evidence.alpha_lookup_uses()?);
        evidence.append_pixel_uses(&mut pixels)?;
        Ok(Self {
            level,
            width,
            height,
            pixels: uses.origin(&pixels, true),
            inventory: uses.freeze()?,
        })
    }
}

fn origin(
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
    fields: &[DdsHeaderField],
    role: SourceUseRole,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let contributors = header.header_uses(fields, role)?;
    Ok(uses.origin(&contributors, true))
}

fn geometry(
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
) -> Result<[FieldOrigin; 2], SourceEvidenceError> {
    Ok([
        origin(
            header,
            uses,
            &[Width, MipCount, Faces, ArraySize, Depth],
            Conversion,
        )?,
        origin(
            header,
            uses,
            &[Height, MipCount, Faces, ArraySize, Depth],
            Conversion,
        )?,
    ])
}
