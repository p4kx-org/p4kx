// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::tag::Tag;
use crate::{GfxError, RgbaImage};

/// A bitmap a movie defines under an ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap<'a> {
    /// The ID of the bitmap.
    pub id: u16,
    /// Where and how its pixels are stored.
    pub source: BitmapSource<'a>,
}

/// Where and how the pixels of a bitmap are stored.
#[derive(Debug, Clone, PartialEq)]
pub enum BitmapSource<'a> {
    /// zlib-compressed pixels of a `DefineBitsLossless` or `DefineBitsLossless2` tag.
    Lossless(Lossless<'a>),
    /// A JPEG, PNG or GIF image of a `DefineBits`, `DefineBitsJPEG2`, `DefineBitsJPEG3` or
    /// `DefineBitsJPEG4` tag.
    Jpeg(Jpeg<'a>),
    /// An image file next to the movie, of a GFx `DefineExternalImage` or `DefineExternalImage2` tag.
    External(ExternalImage),
    /// A rectangle of another bitmap, of a GFx `DefineSubImage` tag.
    SubImage(SubImage),
}

/// The zlib-compressed pixels of a lossless bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lossless<'a> {
    /// The pixel format: 3 colour-mapped, 4 15-bit RGB, 5 32-bit.
    pub format: u8,
    /// The width in pixels.
    pub width: u16,
    /// The height in pixels.
    pub height: u16,
    /// Whether the pixels and colour table carry premultiplied alpha (`DefineBitsLossless2`).
    pub has_alpha: bool,
    /// The entries of the colour table of a colour-mapped bitmap.
    pub color_table_len: u16,
    /// The zlib stream of the colour table and the pixels.
    pub data: &'a [u8],
}

/// An embedded JPEG, PNG or GIF image.
#[derive(Debug, Clone, PartialEq)]
pub struct Jpeg<'a> {
    /// The tag: 1 `DefineBits`, which needs the movie's JPEG tables, 2 `DefineBitsJPEG2`, 3
    /// `DefineBitsJPEG3`, 4 `DefineBitsJPEG4`.
    pub version: u8,
    /// The image data.
    pub data: &'a [u8],
    /// The zlib-compressed alpha channel, one byte per pixel.
    pub alpha: Option<&'a [u8]>,
    /// The deblocking filter strength of `DefineBitsJPEG4`.
    pub deblocking: Option<f32>,
}

/// The format of the image data of an embedded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmbeddedFormat {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// GIF.
    Gif,
}

impl Jpeg<'_> {
    /// Returns the format of the image data, told by its first bytes.
    #[must_use]
    pub fn format(&self) -> EmbeddedFormat {
        if self.data.starts_with(b"\x89PNG") {
            EmbeddedFormat::Png
        } else if self.data.starts_with(b"GIF8") {
            EmbeddedFormat::Gif
        } else {
            EmbeddedFormat::Jpeg
        }
    }
}

/// An image file a GFx movie refers to instead of embedding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImage {
    /// The file format.
    pub format: ImageFileFormat,
    /// The width the image is shown with, in pixels.
    pub target_width: u16,
    /// The height the image is shown with, in pixels.
    pub target_height: u16,
    /// The name the image is exported under.
    pub export_name: String,
    /// The path of the file, relative to the movie.
    pub file_name: String,
}

/// A rectangle of another bitmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubImage {
    /// The ID of the bitmap it cuts.
    pub image_id: u16,
    /// The left edge in pixels.
    pub x1: u16,
    /// The top edge in pixels.
    pub y1: u16,
    /// The right edge in pixels.
    pub x2: u16,
    /// The bottom edge in pixels.
    pub y2: u16,
}

/// The file format of a GFx external image, as the GFx exporter numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageFileFormat(pub u16);

impl ImageFileFormat {
    /// The DDS format, whose image a texture of the Archive holds.
    pub const DDS: Self = Self(14);

    /// Returns the name of the format, `None` for a number this crate does not know.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            0 => "default",
            10 => "jpeg",
            11 => "png",
            12 => "gif",
            13 => "tga",
            14 => "dds",
            15 => "hdr",
            16 => "bmp",
            17 => "dib",
            18 => "pfm",
            19 => "tiff",
            _ => return None,
        })
    }
}

/// Reads `DefineBits`, `DefineBitsJPEG2`, `DefineBitsJPEG3` or `DefineBitsJPEG4`.
pub(crate) fn read_jpeg<'a>(tag: &Tag<'a>, version: u8) -> Result<Bitmap<'a>, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    let (data, alpha, deblocking) = if version >= 3 {
        let image_len = usize::try_from(reader.u32()?).unwrap_or(usize::MAX);
        let deblocking = if version >= 4 {
            Some(reader.fixed8()?)
        } else {
            None
        };
        let data = reader.bytes(image_len)?;
        let alpha = reader.rest();
        (data, (!alpha.is_empty()).then_some(alpha), deblocking)
    } else {
        (reader.rest(), None, None)
    };
    Ok(Bitmap {
        id,
        source: BitmapSource::Jpeg(Jpeg {
            version,
            data,
            alpha,
            deblocking,
        }),
    })
}

/// Reads `DefineBitsLossless` or `DefineBitsLossless2`.
pub(crate) fn read_lossless<'a>(tag: &Tag<'a>, has_alpha: bool) -> Result<Bitmap<'a>, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    let format = reader.u8()?;
    let width = reader.u16()?;
    let height = reader.u16()?;
    let color_table_len = if format == 3 {
        u16::from(reader.u8()?) + 1
    } else {
        0
    };
    Ok(Bitmap {
        id,
        source: BitmapSource::Lossless(Lossless {
            format,
            width,
            height,
            has_alpha,
            color_table_len,
            data: reader.rest(),
        }),
    })
}

/// Reads `DefineExternalImage`, or `DefineExternalImage2` with its ID type after the ID.
pub(crate) fn read_external<'a>(tag: &Tag<'a>, version: u8) -> Result<Bitmap<'a>, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    if version >= 2 {
        reader.u16()?;
    }
    let format = ImageFileFormat(reader.u16()?);
    let target_width = reader.u16()?;
    let target_height = reader.u16()?;
    let export_name = reader.short_string()?;
    let file_name = if reader.remaining() > 0 {
        reader.short_string()?
    } else {
        String::new()
    };
    Ok(Bitmap {
        id,
        source: BitmapSource::External(ExternalImage {
            format,
            target_width,
            target_height,
            export_name,
            file_name,
        }),
    })
}

/// Reads `DefineSubImage`.
pub(crate) fn read_sub_image<'a>(tag: &Tag<'a>) -> Result<Bitmap<'a>, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    Ok(Bitmap {
        id,
        source: BitmapSource::SubImage(SubImage {
            image_id: reader.u16()?,
            x1: reader.u16()?,
            y1: reader.u16()?,
            x2: reader.u16()?,
            y2: reader.u16()?,
        }),
    })
}

/// A decoded embedded bitmap and the table input used by its decoder.
#[derive(Debug)]
#[non_exhaustive]
pub struct DecodedBitmap {
    /// Straight-alpha RGBA pixels.
    pub image: RgbaImage,
    /// Whether the decoder used the movie's admitted shared JPEG tables.
    pub used_jpeg_tables: bool,
}

impl DecodedBitmap {
    pub(crate) const fn without_tables(image: RgbaImage) -> Self {
        Self {
            image,
            used_jpeg_tables: false,
        }
    }
}
