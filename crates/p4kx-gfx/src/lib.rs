// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the GFx and SWF user-interface movies of Star Citizen Archives: the file and movie
//! header with zlib or LZMA compression, the tag stream with unknown tags kept, and the shapes,
//! morph shapes, bitmaps (embedded, GFx external and sub-images), fonts and sprites a movie
//! defines. Lossless and embedded bitmaps decode to RGBA and write as PNG; shapes serialise to
//! JSON in the units the file stores.
//!
//! The first frame of a movie composes into a [`Scene`] independent of any backend, which
//! [`Scene::rasterize`] draws with tiny-skia and [`Scene::write_svg`] writes as SVG under the same
//! budgets. Static texts and text fields compose into glyph shapes of the scene, so both
//! backends draw the same outlines (as paths in SVG); a missing font or glyph is drawn as an
//! empty box and listed, and [`TextOptions`] adds fonts of other movies and the localisation of
//! `@key` texts. No system font is loaded.
//!
//! Every size is checked against the data left and the limits [`MAX_MOVIE_LEN`],
//! [`MAX_BITMAP_PIXELS`] and [`MAX_SHAPE_RECORDS`], a scene against [`MAX_SPRITE_DEPTH`],
//! [`MAX_SCENE_PLACEMENTS`], [`MAX_SCENE_PATHS`], [`MAX_SCENE_SEGMENTS`], [`MAX_SCENE_RECORDS`],
//! [`MAX_SCENE_MASKS`], [`MAX_SCENE_GLYPHS`] and [`MAX_GRADIENT_STOPS`], the static texts of a
//! movie against [`MAX_TEXT_GLYPHS`], a text field against [`MAX_TEXT_CHARS`], a canvas against [`MAX_CANVAS_SIDE`],
//! [`MAX_CANVAS_PIXELS`] and the fill area [`MAX_COVERAGE_FACTOR`] times its size, an SVG document
//! against [`MAX_SVG_BYTES`];
//! corrupt input yields a [`GfxError`] or a skipped element, never a panic.

mod asset_origins;
mod assets;
mod bitmap;
mod embedded_png;
mod error;
mod face;
mod font;
mod font_info;
mod geometry;
mod header;
mod image;
mod import;
mod jpeg;
mod json;
#[cfg(test)]
mod limit_tests;
mod line;
mod lossless;
mod morph;
mod morph_frame;
mod movie;
mod movie_source;
mod place;
mod raster;
mod raster_budget;
mod raster_mask;
mod raster_paint;
mod reader;
mod scene;
mod scene_compose;
mod scene_edit_text;
mod scene_report;
mod scene_shape;
mod scene_text;
mod shape;
mod style;
mod svg;
mod svg_budget;
mod svg_layer;
#[cfg(test)]
mod svg_limit_tests;
mod svg_plan;
mod svg_text;
mod tag;
mod text;
mod text_fonts;
mod text_html;
mod text_layout;
#[cfg(test)]
mod text_limit_tests;
mod text_loc;
mod timeline;
mod walker;

pub use asset_origins::{
    AssetOrigins, FailureOrigin, FontOrigins, LabelOrigin, SpanRun, SpanRuns, SpriteOrigins,
    TimelineOrigins,
};
pub use assets::{Assets, OtherDefinition, TagCount, TagFailure, UnknownTag};
pub use bitmap::{
    Bitmap, BitmapSource, DecodedBitmap, EmbeddedFormat, ExternalImage, ImageFileFormat, Jpeg,
    Lossless, SubImage,
};
pub use error::GfxError;
pub use face::{FontFace, MISSING_GLYPH_EM, missing_glyph};
pub use font::{Font, FontFormat, FontLayout, Glyph, Kerning};
pub use geometry::{Matrix, Point, Rect, Rgba};
pub use header::{Compression, Header, MAX_MOVIE_LEN, Signature};
pub use image::{MAX_BITMAP_PIXELS, RgbaImage};
pub use import::{Import, ImportedAsset, MAX_IMPORTS, ScalingGrid, SymbolClass};
pub use morph::MorphShape;
pub use movie::{Movie, ObservedAssets};
pub use movie_source::{MovieSource, MovieSpan, TagItemSite, TagSite};
pub use place::{ColorTransform, Filter, FilterKind, Placement};
pub use raster::{
    Background, CanvasSize, MAX_CANVAS_PIXELS, MAX_CANVAS_SIDE, ObservedRaster, Raster,
    RasterOptions,
};
pub use raster_budget::{MAX_COVERAGE_FACTOR, MAX_SVG_BYTES};
pub use scene::{
    Affine, BitmapPaint, Cap, GradientKind, GradientPaint, Join, LayerKind, MAX_GRADIENT_STOPS,
    MAX_SCENE_MASKS, MAX_SCENE_PATHS, MAX_SCENE_PLACEMENTS, MAX_SCENE_RECORDS, MAX_SCENE_SEGMENTS,
    Origin, Paint, PathCommand, Scene, SceneItem, SceneMask, ScenePath, ScenePoint, SceneShape,
    ShapeLayer, Spread, StrokeStyle,
};
pub use scene_report::{MAX_SPRITE_DEPTH, SkipReason, Skipped};
pub use scene_text::MAX_SCENE_GLYPHS;
pub use shape::{MAX_SHAPE_RECORDS, NewStyles, Shape, ShapeRecord, StyleChange};
pub use style::{FillStyle, Gradient, GradientStop, LineStyle};
pub use svg::{ObservedSvgReport, SvgReport};
pub use tag::tag_name;
pub use text::{EditLayout, EditText, GlyphEntry, MAX_TEXT_GLYPHS, StaticText, TextRecord};
pub use text_fonts::{Localise, TextOptions};
pub use text_html::MAX_TEXT_CHARS;
pub use text_loc::{Localised, localise_text};
pub use timeline::{
    Export, ExporterInfo, FrameLabel, MAX_FRAME_BYTES, MAX_FRAME_PLACEMENTS, Sprite, Timeline,
};
