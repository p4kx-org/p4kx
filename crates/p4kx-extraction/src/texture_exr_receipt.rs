// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_texture::{DdsHeaderField, HdrImage, MipPayloadLocation, write_exr};

use super::{CUBE_FACES, ExrOrigins};
use crate::output_file::write_error;
use crate::texture_preview_png::input::PreviewInputs;
use crate::textures::image_receipt;
use crate::{
    ConversionError, FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUseRole,
    SourceUses, Texture,
};

pub(super) fn publish(
    texture: &Texture<'_>,
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    out: &Path,
    level: u32,
    cross: bool,
    files: &[PathBuf],
    images: &[HdrImage],
) -> Result<ExrOrigins, ConversionError> {
    let (fields, inventory) =
        summary(texture, inputs, cross, files.len()).map_err(|error| write_error(out, error))?;
    let receipts = image_receipt::prepare(texture, files)?;
    let surfaces = if cross { CUBE_FACES as u64 } else { 1 };
    for (index, ((file, image), receipt)) in files.iter().zip(images).zip(&receipts).enumerate() {
        let first = index as u64 * surfaces;
        let (origin, inventory) =
            image_receipt::image_origin(inputs, level, first..first + surfaces, |_, _| Ok(()))
                .map_err(|error| write_error(file, error))?;
        image_receipt::publish(receipt, file, &origin, &inventory, |writer| {
            write_exr(image, writer)
        })?;
    }
    Ok(ExrOrigins::of_images(&receipts, fields, inventory))
}

fn summary(
    texture: &Texture<'_>,
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    cross: bool,
    image_count: usize,
) -> io::Result<(FieldOrigins, SourceInventory)> {
    use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};

    let uses = SourceUses::with_compact_use_refs();
    let opened = texture
        .preview_header_evidence(&uses)
        .map_err(io::Error::other)?;
    let (decoded, complete) = inputs
        .color_header_evidence(&uses)
        .map_err(io::Error::other)?;
    let mut fields = FieldOrigins::new();
    insert(
        &mut fields,
        "/texture",
        opened.path_origin(&uses).map_err(io::Error::other)?,
    )?;
    let format = opened
        .header_uses(&[Prefix, Format], SourceUseRole::Selector)
        .map_err(io::Error::other)?;
    insert(&mut fields, "/format", uses.origin(&format, complete))?;
    for (path, dimension) in [("/width", Width), ("/height", Height)] {
        let mut contributors = decoded
            .header_uses(&[Prefix, dimension], SourceUseRole::Conversion)
            .map_err(io::Error::other)?;
        if cross {
            contributors.extend(
                opened
                    .header_uses(&[Faces], SourceUseRole::Selector)
                    .map_err(io::Error::other)?,
            );
        }
        insert(&mut fields, path, uses.origin(&contributors, complete))?;
    }
    let layout = opened
        .header_uses(
            &[Prefix, Format, MipCount, Faces, ArraySize, Depth],
            SourceUseRole::Selector,
        )
        .map_err(io::Error::other)?;
    let files = uses.origin(&layout, complete);
    insert(&mut fields, "/files", files.clone())?;
    for index in 0..image_count {
        insert(&mut fields, &format!("/files/{index}"), files.clone())?;
    }
    Ok((fields, uses.freeze().map_err(io::Error::other)?))
}

fn insert(fields: &mut FieldOrigins, path: &str, origin: FieldOrigin) -> io::Result<()> {
    let path = FieldPath::parse(path).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
