// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_foundation::ParseError;
use thiserror::Error;

/// A failure to read a GFx or SWF movie or to decode or write one of its assets.
#[derive(Debug, Error)]
pub enum GfxError {
    /// The data ended before a value could be read completely.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// The file starts with none of the signatures `FWS`, `CWS`, `ZWS`, `GFX` and `CFX`.
    #[error("not a GFx or SWF movie: it starts with {found:02X?}")]
    Signature {
        /// The first bytes of the file.
        found: Vec<u8>,
    },
    /// The header declares a movie shorter than the header itself.
    #[error("the header declares a movie of {declared} bytes, shorter than the header")]
    DeclaredLength {
        /// The declared length.
        declared: u32,
    },
    /// The movie decompresses to more than the limit.
    #[error("the movie decompresses to more than {limit} bytes")]
    TooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// The compressed movie is corrupt or truncated.
    #[error("cannot decompress the movie: {message}")]
    Decompress {
        /// What the decompressor reported.
        message: String,
    },
    /// A bit field is wider than 32 bits.
    #[error("a bit field of {bits} bits is wider than 32 bits")]
    BitCount {
        /// The width in bits.
        bits: u32,
    },
    /// A tag declares a body longer than the data left.
    #[error("tag {code} at offset {offset} declares {len} bytes, but only {available} are left")]
    TagLength {
        /// The tag code.
        code: u16,
        /// The offset of the tag header in the movie data.
        offset: usize,
        /// The declared body length.
        len: u32,
        /// The bytes left.
        available: usize,
    },
    /// The shapes of the movie hold more records than the limit.
    #[error("the shapes of the movie hold more than {limit} records")]
    TooManyRecords {
        /// The limit.
        limit: usize,
    },
    /// The static texts of the movie hold more glyph entries than the limit.
    #[error("the static texts of the movie hold more than {limit} glyphs")]
    TooManyTextGlyphs {
        /// The limit.
        limit: usize,
    },
    /// The movie imports more assets than the limit.
    #[error("the movie imports more than {limit} assets")]
    TooManyImports {
        /// The limit.
        limit: usize,
    },
    /// The display lists of the frames of the movie hold more placements than the limit.
    #[error(
        "the display lists of the frames hold more than {limit} placements; later frames are not kept"
    )]
    TooManyFramePlacements {
        /// The limit.
        limit: usize,
    },
    /// The display lists of the frames occupy more bytes than the limit.
    #[error(
        "the display lists of the frames occupy more than {limit} bytes; later frames are not kept"
    )]
    FrameListsTooLarge {
        /// The limit in bytes.
        limit: usize,
    },
    /// The offset of the end edges of a morph shape lies beyond its tag.
    #[error("the end edges of the morph shape start beyond its tag, at offset {offset}")]
    MorphEndEdges {
        /// The offset the tag declares.
        offset: u32,
    },
    /// A glyph or the code table of a font starts beyond its tag.
    #[error("a glyph or the code table of the font starts beyond its tag, at offset {offset}")]
    GlyphOffset {
        /// The offset from the start of the offset table.
        offset: usize,
    },
    /// A fill style has an unknown type.
    #[error("unknown fill style type {kind:#04X}")]
    FillStyle {
        /// The type byte.
        kind: u8,
    },
    /// The stage of the movie has no area.
    #[error("the stage of the movie has no area")]
    EmptyStage,
    /// The canvas to render on is empty or too large.
    #[error(
        "a canvas of {width} x {height} pixels is empty or larger than allowed (at most {} pixels a side and {} pixels)",
        crate::MAX_CANVAS_SIDE,
        crate::MAX_CANVAS_PIXELS
    )]
    CanvasSize {
        /// The width in pixels.
        width: u32,
        /// The height in pixels.
        height: u32,
    },
    /// A filter of a placement has an unknown type.
    #[error("unknown filter type {kind}")]
    Filter {
        /// The type byte.
        kind: u8,
    },
    /// A bitmap has more pixels than the limit.
    #[error("the bitmap has {width} x {height} pixels, more than {limit}")]
    BitmapTooLarge {
        /// The width in pixels.
        width: u64,
        /// The height in pixels.
        height: u64,
        /// The limit in pixels.
        limit: u64,
    },
    /// A lossless bitmap has an unknown pixel format.
    #[error("unknown lossless bitmap format {format}")]
    LosslessFormat {
        /// The format byte.
        format: u8,
    },
    /// The compressed pixels or alpha of a bitmap are corrupt or too short.
    #[error("the bitmap data is corrupt or too short: {message}")]
    BitmapData {
        /// What is wrong.
        message: String,
    },
    /// An embedded JPEG cannot be decoded.
    #[error("cannot decode the JPEG: {message}")]
    Jpeg {
        /// What the decoder reported.
        message: String,
    },
    /// An embedded PNG cannot be decoded.
    #[error("cannot decode the embedded PNG: {message}")]
    Png {
        /// What the decoder reported.
        message: String,
    },
    /// The bitmap holds a GIF, which is not supported.
    #[error("embedded GIF images are not supported")]
    Gif,
    /// The bitmap is stored outside the movie or cuts another bitmap.
    #[error("the bitmap is not stored in the movie")]
    NotEmbedded,
    /// An output could not be written.
    #[error("cannot write the output")]
    Write(#[source] io::Error),
    /// The SVG document would be larger than the limit.
    #[error("the SVG document would be larger than {limit} bytes")]
    SvgTooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// The image could not be encoded as PNG.
    #[error("cannot encode the PNG: {0}")]
    EncodePng(String),
}
