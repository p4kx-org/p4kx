// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::panic::{AssertUnwindSafe, catch_unwind};

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;
use zune_jpeg::errors::DecodeErrors;

use crate::GfxError;
use crate::bitmap::{DecodedBitmap, EmbeddedFormat, Jpeg};
use crate::embedded_png::{decode_png, png_size};
use crate::image::{RgbaImage, checked_pixels, inflate_exact};
use crate::lossless::unmultiply;

const END_THEN_START: [u8; 4] = [0xFF, 0xD9, 0xFF, 0xD8];
const MAX_SIDE: usize = 16_384;

/// Decodes an embedded JPEG or PNG to straight-alpha RGBA, with the JPEG tables of the movie for
/// a `DefineBits` image and the alpha channel of a `DefineBitsJPEG3` or `DefineBitsJPEG4` image,
/// whose colours are premultiplied.
pub(crate) fn decode_jpeg(
    jpeg: &Jpeg<'_>,
    tables: Option<&[u8]>,
) -> Result<DecodedBitmap, GfxError> {
    match jpeg.format() {
        EmbeddedFormat::Gif => Err(GfxError::Gif),
        EmbeddedFormat::Png => decode_png(jpeg.data).map(DecodedBitmap::without_tables),
        EmbeddedFormat::Jpeg => {
            let tables = own_tables(jpeg, tables);
            let data = without_end_then_start(tables, jpeg.data);
            let mut image = decode_jpeg_data(&data)?;
            if let Some(alpha) = jpeg.alpha {
                apply_alpha(&mut image, alpha)?;
            }
            Ok(DecodedBitmap {
                image,
                used_jpeg_tables: !tables.is_empty(),
            })
        }
    }
}

/// Returns the width and height an embedded JPEG or PNG states, read from its headers only.
pub(crate) fn jpeg_size(jpeg: &Jpeg<'_>, tables: Option<&[u8]>) -> Result<(u32, u32), GfxError> {
    match jpeg.format() {
        EmbeddedFormat::Gif => Err(GfxError::Gif),
        EmbeddedFormat::Png => png_size(jpeg.data),
        EmbeddedFormat::Jpeg => {
            let data = without_end_then_start(own_tables(jpeg, tables), jpeg.data);
            let mut decoder = decoder(&data);
            read_size(&mut decoder)
        }
    }
}

fn own_tables<'t>(jpeg: &Jpeg<'_>, tables: Option<&'t [u8]>) -> &'t [u8] {
    tables.filter(|_| jpeg.version == 1).unwrap_or_default()
}

/// Returns `tables` followed by `data` without the end-and-start marker pairs `FF D9 FF D8` that
/// SWF files put before an image and between the JPEG tables and an image.
fn without_end_then_start<'d>(tables: &[u8], data: &'d [u8]) -> Cow<'d, [u8]> {
    let found = data
        .windows(END_THEN_START.len())
        .position(|window| window == END_THEN_START);
    let start = match (tables.is_empty(), found) {
        (true, None) => return Cow::Borrowed(data),
        (true, Some(found)) => found,
        (false, _) => 0,
    };
    let (head, rest) = data.split_at_checked(start).unwrap_or((&[], data));
    let mut cleaned = Vec::with_capacity(tables.len() + data.len());
    cleaned.extend_from_slice(head);
    for &byte in tables.iter().chain(rest) {
        cleaned.push(byte);
        if cleaned.ends_with(&END_THEN_START) {
            cleaned.truncate(cleaned.len() - END_THEN_START.len());
        }
    }
    Cow::Owned(cleaned)
}

fn decoder(data: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_max_width(MAX_SIDE)
        .set_max_height(MAX_SIDE)
        .set_strict_mode(false);
    JpegDecoder::new_with_options(ZCursor::new(data), options)
}

/// Runs a step of the JPEG decoder, turning a panic of the decoder into an error.
fn guarded<T>(step: impl FnOnce() -> Result<T, DecodeErrors>) -> Result<T, GfxError> {
    match catch_unwind(AssertUnwindSafe(step)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(GfxError::Jpeg {
            message: error.to_string(),
        }),
        Err(_) => Err(GfxError::Jpeg {
            message: "the JPEG decoder panicked".to_owned(),
        }),
    }
}

fn read_size(decoder: &mut JpegDecoder<ZCursor<&[u8]>>) -> Result<(u32, u32), GfxError> {
    guarded(|| decoder.decode_headers())?;
    decoder
        .info()
        .map(|info| (u32::from(info.width), u32::from(info.height)))
        .ok_or_else(|| GfxError::Jpeg {
            message: "the JPEG has no frame header".to_owned(),
        })
}

fn decode_jpeg_data(data: &[u8]) -> Result<RgbaImage, GfxError> {
    let mut decoder = decoder(data);
    let (width, height) = read_size(&mut decoder)?;
    let pixels = checked_pixels(u64::from(width), u64::from(height))?;
    let rgba = guarded(|| decoder.decode())?;
    if rgba.len() != pixels * 4 {
        return Err(GfxError::Jpeg {
            message: format!("{} bytes decoded for {width} x {height} pixels", rgba.len()),
        });
    }
    Ok(RgbaImage::new(width, height, rgba))
}

fn apply_alpha(image: &mut RgbaImage, alpha: &[u8]) -> Result<(), GfxError> {
    let pixels = checked_pixels(u64::from(image.width()), u64::from(image.height()))?;
    let alpha = inflate_exact(alpha, pixels)?;
    for (pixel, alpha) in image
        .pixels_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(alpha)
    {
        let [red, green, blue, _] = *pixel;
        *pixel = unmultiply(red, green, blue, alpha);
    }
    Ok(())
}
