// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::GfxError;
use crate::bitmap::{Bitmap, BitmapSource, DecodedBitmap};
use crate::font::Font;
use crate::geometry::Rgba;
use crate::image::RgbaImage;
use crate::import::{Import, ScalingGrid, SymbolClass};
use crate::jpeg::{decode_jpeg, jpeg_size};
use crate::lossless::decode_lossless;
use crate::morph::MorphShape;
use crate::shape::Shape;
use crate::tag::tag_name;
use crate::text::{EditText, StaticText};
use crate::timeline::{Export, ExporterInfo, Sprite, Timeline};

/// Everything a movie defines, read from its tags and the tags of its sprites.
///
/// A definition whose tag cannot be read is left out and reported in [`Assets::failures`];
/// the other tags are still read.
#[derive(Debug, Default)]
pub struct Assets<'a> {
    /// How often each tag code occurs, sprites included, in ascending code order.
    pub tag_counts: Vec<TagCount>,
    /// How often each tag whose code is known but whose content is not read occurs, sprites
    /// included, in ascending code order.
    pub unread_tags: Vec<TagCount>,
    /// The tags with a code this crate does not know, kept unread in file order.
    pub unknown_tags: Vec<UnknownTag<'a>>,
    /// The main timeline.
    pub timeline: Timeline,
    /// The shapes in file order.
    pub shapes: Vec<Shape>,
    /// The morph shapes in file order.
    pub morph_shapes: Vec<MorphShape>,
    /// The bitmaps in file order.
    pub bitmaps: Vec<Bitmap<'a>>,
    /// The fonts in file order.
    pub fonts: Vec<Font>,
    /// The sprites in file order, nested ones after their parent.
    pub sprites: Vec<Sprite>,
    /// The export names in file order.
    pub exports: Vec<Export>,
    /// The assets the movie imports from other movies, in file order.
    pub imports: Vec<Import>,
    /// The names bound to classes, in file order.
    pub symbol_classes: Vec<SymbolClass>,
    /// The nine-slice grids in file order.
    pub scaling_grids: Vec<ScalingGrid>,
    /// What the GFx exporter states about the movie.
    pub exporter_info: Option<ExporterInfo>,
    /// The JPEG tables `DefineBits` images share.
    pub jpeg_tables: Option<&'a [u8]>,
    /// The static texts in file order.
    pub static_texts: Vec<StaticText>,
    /// The text fields in file order.
    pub edit_texts: Vec<EditText>,
    /// The buttons and videos in file order, whose tags are not read beyond their ID.
    pub other_definitions: Vec<OtherDefinition>,
    /// The background colour of the first `SetBackgroundColor` tag.
    pub background: Option<Rgba>,
    /// The tags that could not be read.
    pub failures: Vec<TagFailure>,
}

/// A definition whose tag is not read beyond its ID: a button or a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OtherDefinition {
    /// The ID of the definition.
    pub id: u16,
    /// The code of its tag.
    pub code: u16,
}

/// How often one tag code occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TagCount {
    /// The tag code.
    pub code: u16,
    /// The number of tags.
    pub count: u32,
}

impl TagCount {
    /// Returns the name of the tag, `None` for a code this crate does not know.
    #[must_use]
    pub const fn name(&self) -> Option<&'static str> {
        tag_name(self.code)
    }
}

/// A tag with a code this crate does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnknownTag<'a> {
    /// The tag code.
    pub code: u16,
    /// The offset of its body in the uncompressed movie.
    pub offset: usize,
    /// The body.
    pub body: &'a [u8],
}

/// A tag that could not be read.
#[derive(Debug)]
pub struct TagFailure {
    /// The tag code.
    pub code: u16,
    /// The offset of its body, or of its header when that is broken, in the uncompressed movie.
    pub offset: usize,
    /// What is wrong.
    pub error: GfxError,
}

impl Assets<'_> {
    /// Returns the first name the definition `id` is exported under.
    #[must_use]
    pub fn export_name(&self, id: u16) -> Option<&str> {
        self.exports
            .iter()
            .find(|export| export.id == id)
            .map(|export| export.name.as_str())
    }

    /// Returns the name and the URL of the movie the definition `id` is imported from.
    #[must_use]
    pub fn import_of(&self, id: u16) -> Option<(&str, &str)> {
        self.imports.iter().find_map(|import| {
            import
                .assets
                .iter()
                .find(|asset| asset.id == id)
                .map(|asset| (asset.name.as_str(), import.url.as_str()))
        })
    }

    /// Returns the width and height of a lossless or embedded bitmap, read from its headers only.
    ///
    /// # Errors
    ///
    /// Returns [`GfxError::NotEmbedded`] for an external image or a sub-image, [`GfxError::Gif`]
    /// for a GIF, and another [`GfxError`] when the headers are corrupt.
    pub fn bitmap_size(&self, bitmap: &Bitmap<'_>) -> Result<(u32, u32), GfxError> {
        match &bitmap.source {
            BitmapSource::Lossless(lossless) => {
                Ok((u32::from(lossless.width), u32::from(lossless.height)))
            }
            BitmapSource::Jpeg(jpeg) => jpeg_size(jpeg, self.jpeg_tables),
            BitmapSource::External(_) | BitmapSource::SubImage(_) => Err(GfxError::NotEmbedded),
        }
    }

    /// Decodes a lossless or embedded bitmap to straight-alpha RGBA.
    ///
    /// # Errors
    ///
    /// Returns [`GfxError::NotEmbedded`] for an external image or a sub-image,
    /// [`GfxError::BitmapTooLarge`] above [`crate::MAX_BITMAP_PIXELS`], [`GfxError::Gif`] for a
    /// GIF, and another [`GfxError`] when the data is corrupt.
    pub fn decode_bitmap(&self, bitmap: &Bitmap<'_>) -> Result<RgbaImage, GfxError> {
        self.decode_bitmap_observed(bitmap)
            .map(|bitmap| bitmap.image)
    }

    /// Decodes an embedded bitmap and retains whether shared JPEG tables contributed.
    ///
    /// # Errors
    ///
    /// Returns the errors of an embedded bitmap decode.
    pub fn decode_bitmap_observed(&self, bitmap: &Bitmap<'_>) -> Result<DecodedBitmap, GfxError> {
        match &bitmap.source {
            BitmapSource::Lossless(lossless) => {
                decode_lossless(lossless).map(DecodedBitmap::without_tables)
            }
            BitmapSource::Jpeg(jpeg) => decode_jpeg(jpeg, self.jpeg_tables),
            BitmapSource::External(_) | BitmapSource::SubImage(_) => Err(GfxError::NotEmbedded),
        }
    }
}
