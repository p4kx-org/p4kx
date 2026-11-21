// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use png::{
    BitDepth, ColorType, Compression, Encoder, EncodingError, ScaledFloat, SrgbRenderingIntent,
};

use crate::{DdsTransfer, RgbaImage, TextureError};

/// Writes `image` to `out` as an 8-bit RGBA PNG.
///
/// # Errors
///
/// Returns [`TextureError::Write`] when `out` fails and [`TextureError::EncodePng`] when the
/// image cannot be encoded.
pub fn write_png<W: Write>(image: &RgbaImage, out: W) -> Result<(), TextureError> {
    write_png_with_transfer(image, DdsTransfer::Unknown, out)
}

/// Writes RGBA samples unchanged, with a known RGB transfer declaration when supplied.
///
/// # Errors
/// Returns an output or PNG encoding error.
pub fn write_png_with_transfer<W: Write>(
    image: &RgbaImage,
    transfer: DdsTransfer,
    out: W,
) -> Result<(), TextureError> {
    let mut encoder = Encoder::new(out, image.width(), image.height());
    match transfer {
        DdsTransfer::Srgb => encoder.set_source_srgb(SrgbRenderingIntent::RelativeColorimetric),
        DdsTransfer::Linear => encoder.set_source_gamma(ScaledFloat::new(1.0)),
        DdsTransfer::Unknown => {}
    }
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(Compression::Fast);
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(image.pixels()).map_err(png_error)?;
    writer.finish().map_err(png_error)
}

fn png_error(error: EncodingError) -> TextureError {
    match error {
        EncodingError::IoError(source) => TextureError::Write(source),
        other => TextureError::EncodePng(other.to_string()),
    }
}
