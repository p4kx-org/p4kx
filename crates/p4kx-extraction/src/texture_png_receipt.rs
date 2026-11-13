// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_texture::{
    DdsHeaderField, DdsPngEncoding, MipPayloadLocation, RgbaImage, write_png_with_transfer,
};

use crate::output_file::write_error;
use crate::texture_preview_png::input::PreviewInputs;
use crate::{
    CUBE_FACE_NAMES, ConversionError, CubemapLayout, FieldOrigin, FieldOrigins, FieldPath,
    FileReceipt, OriginOutputAddress, SourceInventory, SourceUseRole, SourceUses, Texture,
    write_origin_companion,
};

/// Per-file receipt destinations and exact origin for PNG summary values.
#[derive(Debug)]
pub struct PngOrigins {
    /// Sidecar paths in the order of the output images.
    pub receipts: Vec<PathBuf>,
    /// Sources of the output transfer and channel transform.
    pub fields: FieldOrigins,
    /// Original evidence for the report fields.
    pub inventory: SourceInventory,
}

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
        encoding: DdsPngEncoding,
        destinations: &[PathBuf],
        out: &Path,
    ) -> Result<Self, ConversionError> {
        let protection = texture.write_protection()?;
        let receipts = destinations
            .iter()
            .map(|destination| {
                let receipt = FileReceipt::new(destination, protection, protection)
                    .map_err(|error| write_error(destination, error))?;
                texture.refuse_input(receipt.path())?;
                Ok(receipt)
            })
            .collect::<Result<Vec<_>, ConversionError>>()?;
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
        encoding: DdsPngEncoding,
        files: &[PathBuf],
        images: &[RgbaImage],
    ) -> Result<(), ConversionError> {
        for (position, ((file, image), receipt)) in
            files.iter().zip(images).zip(&self.receipts).enumerate()
        {
            let first = (position * self.surfaces) as u64;
            let surfaces = first..first + self.surfaces as u64;
            let (origin, inventory) = image_origin(inputs, level, encoding, surfaces)
                .map_err(|error| write_error(file, error))?;
            write(receipt, file, image, encoding, &origin, &inventory)?;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> PngOrigins {
        PngOrigins {
            receipts: self
                .receipts
                .iter()
                .map(|receipt| receipt.path().to_path_buf())
                .collect(),
            fields: self.fields,
            inventory: self.inventory,
        }
    }
}

fn image_origin(
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    level: u32,
    encoding: DdsPngEncoding,
    surfaces: std::ops::Range<u64>,
) -> io::Result<(FieldOrigin, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut contributions = Vec::new();
    let mut complete = true;
    for index in surfaces {
        let observed = inputs
            .register_image(&uses, level, index)
            .map_err(io::Error::other)?
            .ok_or_else(|| io::Error::other("PNG image has no matching DDS surface"))?;
        observed
            .append_pixel_uses(&mut contributions)
            .map_err(io::Error::other)?;
        observed
            .append_output_transfer_uses(&mut contributions, encoding)
            .map_err(io::Error::other)?;
        complete &= observed.is_complete();
    }
    let origin = uses.origin(&contributions, complete);
    Ok((origin, uses.freeze().map_err(io::Error::other)?))
}

fn summary_fields(
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    encoding: DdsPngEncoding,
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
    let mut fields = FieldOrigins::new();
    for (path, contributors) in [("/output_transfer", transfer), ("/png_transform", format)] {
        let path = FieldPath::parse(path).map_err(io::Error::other)?;
        fields
            .insert(path, uses.origin(&contributors, complete))
            .map_err(io::Error::other)?;
    }
    Ok((fields, uses.freeze().map_err(io::Error::other)?))
}

fn write(
    receipt: &FileReceipt<'_>,
    file: &Path,
    image: &RgbaImage,
    encoding: DdsPngEncoding,
    origin: &FieldOrigin,
    inventory: &SourceInventory,
) -> Result<(), ConversionError> {
    receipt
        .publish(
            |writer| {
                write_png_with_transfer(image, encoding.transfer, writer).map_err(|cause| {
                    match cause {
                        p4kx_texture::TextureError::Write(error) => error,
                        other => io::Error::other(other),
                    }
                })
            },
            |writer, binding, ()| {
                write_origin_companion(
                    writer,
                    binding,
                    inventory,
                    [(&OriginOutputAddress::OpaquePayload, origin)],
                )
            },
        )
        .map_err(|error| write_error(file, error))
}
