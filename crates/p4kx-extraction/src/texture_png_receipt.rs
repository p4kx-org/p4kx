// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_texture::{
    DdsAlphaTransform, DdsHeaderField, DdsPngEncoding, MipPayloadLocation, RgbaImage,
    write_png_with_transfer,
};

use crate::output_file::write_error;
use crate::texture_preview_png::input::PreviewInputs;
use crate::textures::image_receipt;
pub use crate::textures::image_receipt::PngOrigins;
use crate::{
    CUBE_FACE_NAMES, ConversionError, CubemapLayout, FieldOrigins, FieldPath, FileReceipt,
    SourceInventory, SourceUseRole, SourceUses, Texture,
};

pub(super) struct PreparedPng<'source> {
    receipts: Vec<FileReceipt<'source>>,
    surfaces: usize,
    fields: FieldOrigins,
    inventory: SourceInventory,
}

impl<'source> PreparedPng<'source> {
    pub(super) fn new(
        texture: &'source Texture<'_>,
        inputs: &PreviewInputs<'_, MipPayloadLocation>,
        layout: CubemapLayout,
        encoding: (DdsPngEncoding, DdsAlphaTransform),
        destinations: &[PathBuf],
        out: &Path,
    ) -> Result<Self, ConversionError> {
        let receipts = image_receipt::prepare(texture, destinations)?;
        let cross = layout == CubemapLayout::Cross
            && texture.header().faces() as usize == CUBE_FACE_NAMES.len();
        let (fields, inventory) =
            summary_fields(inputs, encoding).map_err(|error| write_error(out, error))?;
        Ok(Self {
            receipts,
            surfaces: if cross { CUBE_FACE_NAMES.len() } else { 1 },
            fields,
            inventory,
        })
    }

    pub(super) fn write_images(
        &self,
        inputs: &PreviewInputs<'_, MipPayloadLocation>,
        level: u32,
        encoding: (DdsPngEncoding, DdsAlphaTransform),
        files: &[PathBuf],
        images: &[RgbaImage],
    ) -> Result<(), ConversionError> {
        for (position, ((file, image), receipt)) in
            files.iter().zip(images).zip(&self.receipts).enumerate()
        {
            let first = (position * self.surfaces) as u64;
            let surfaces = first..first + self.surfaces as u64;
            let (origin, inventory) =
                image_receipt::image_origin(inputs, level, surfaces, |observed, uses| {
                    observed.append_output_transfer_uses(uses, encoding.0)?;
                    observed.append_alpha_transform_uses(uses, encoding.1)
                })
                .map_err(|error| write_error(file, error))?;
            image_receipt::publish(receipt, file, &origin, &inventory, |writer| {
                write_png_with_transfer(image, encoding.0.transfer, writer)
            })?;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> PngOrigins {
        PngOrigins::of_images(&self.receipts, self.fields, self.inventory)
    }
}

fn summary_fields(
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    (encoding, alpha): (DdsPngEncoding, DdsAlphaTransform),
) -> io::Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let (header, complete) = inputs
        .color_header_evidence(&uses)
        .map_err(io::Error::other)?;
    let transfer = header
        .output_transfer_uses(encoding)
        .map_err(io::Error::other)?;
    let format = header
        .header_uses(
            &[DdsHeaderField::Prefix, DdsHeaderField::Format],
            SourceUseRole::Conversion,
        )
        .map_err(io::Error::other)?;
    let association = header.alpha_association_uses().map_err(io::Error::other)?;
    let mut fields = FieldOrigins::new();
    if alpha != DdsAlphaTransform::Preserve {
        let mut contributors = association.clone();
        contributors.extend(transfer.iter().cloned());
        fields
            .insert(
                FieldPath::parse("/alpha_transform").map_err(io::Error::other)?,
                uses.origin(&contributors, complete),
            )
            .map_err(io::Error::other)?;
    }
    for (path, contributors) in [
        ("/output_transfer", transfer),
        ("/png_transform", format),
        ("/alpha_association", association),
    ] {
        let path = FieldPath::parse(path).map_err(io::Error::other)?;
        fields
            .insert(path, uses.origin(&contributors, complete))
            .map_err(io::Error::other)?;
    }
    Ok((fields, uses.freeze().map_err(io::Error::other)?))
}
