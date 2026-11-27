// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

use p4kx_texture::{MipPayloadLocation, TextureError};

use crate::output_file::write_error;
use crate::texture_preview_png::input::PreviewInputs;
use crate::{
    ConversionError, FieldOrigin, FieldOrigins, FileReceipt, OriginOutputAddress,
    PreviewPngEvidence, SourceEvidenceError, SourceInventory, SourceUse, SourceUses, Texture,
    write_origin_companion,
};

/// Byte-bound receipt paths and exact origins for a texture image export.
#[derive(Debug)]
pub struct PngOrigins {
    /// Sidecar paths in output image order.
    pub receipts: Vec<PathBuf>,
    /// Original evidence keyed by report field.
    pub fields: FieldOrigins,
    /// The inventory in which the report's references resolve.
    pub inventory: SourceInventory,
}

impl PngOrigins {
    pub(crate) fn of_images(
        receipts: &[FileReceipt<'_>],
        fields: FieldOrigins,
        inventory: SourceInventory,
    ) -> Self {
        Self {
            receipts: receipts
                .iter()
                .map(|receipt| receipt.path().to_path_buf())
                .collect(),
            fields,
            inventory,
        }
    }
}

pub(crate) fn prepare<'texture>(
    texture: &'texture Texture<'_>,
    destinations: &[PathBuf],
) -> Result<Vec<FileReceipt<'texture>>, ConversionError> {
    let protection = texture.write_protection()?;
    destinations
        .iter()
        .map(|destination| {
            let receipt = FileReceipt::new(destination, protection, protection)
                .map_err(|error| write_error(destination, error))?;
            texture.refuse_input(receipt.path())?;
            Ok(receipt)
        })
        .collect()
}

pub(crate) fn image_origin(
    inputs: &PreviewInputs<'_, MipPayloadLocation>,
    level: u32,
    surfaces: Range<u64>,
    append_encoding: impl Fn(
        &PreviewPngEvidence,
        &mut Vec<SourceUse>,
    ) -> Result<(), SourceEvidenceError>,
) -> io::Result<(FieldOrigin, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut contributions = Vec::new();
    let mut complete = true;
    for index in surfaces {
        let observed = inputs
            .register_image(&uses, level, index)
            .map_err(io::Error::other)?
            .ok_or_else(|| io::Error::other("image has no matching DDS surface"))?;
        observed
            .append_pixel_uses(&mut contributions)
            .map_err(io::Error::other)?;
        append_encoding(&observed, &mut contributions).map_err(io::Error::other)?;
        complete &= observed.is_complete();
    }
    let origin = uses.origin(&contributions, complete);
    Ok((origin, uses.freeze().map_err(io::Error::other)?))
}

pub(crate) fn publish(
    receipt: &FileReceipt<'_>,
    file: &Path,
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), TextureError>,
) -> Result<(), ConversionError> {
    receipt
        .publish(
            |writer| {
                encode(writer).map_err(|error| match error {
                    TextureError::Write(error) => error,
                    other => io::Error::other(other),
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
