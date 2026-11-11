// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::raster_budget::MAX_COVERAGE_FACTOR;
use crate::scene::{
    MAX_GRADIENT_STOPS, MAX_SCENE_MASKS, MAX_SCENE_PATHS, MAX_SCENE_PLACEMENTS, MAX_SCENE_RECORDS,
    MAX_SCENE_SEGMENTS,
};
use crate::scene_text::MAX_SCENE_GLYPHS;
use crate::tag::tag_name;
use crate::text_html::MAX_TEXT_CHARS;

/// The deepest nested sprite placement composed into a scene.
pub const MAX_SPRITE_DEPTH: u32 = 16;

/// An element of a frame that was skipped, or drawn without an effect it asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Skipped {
    /// The ID of the definition placed.
    pub id: u16,
    /// The depth of the placement on its timeline.
    pub depth: u16,
    /// The sprite whose timeline holds the placement, `None` for the main timeline.
    pub sprite: Option<u16>,
    /// Why.
    pub reason: SkipReason,
}

/// Why an element was skipped or drawn without an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipReason {
    /// No definition has the ID placed.
    UnknownDefinition,
    /// The definition is of a kind that is not drawn, named by the code of its tag: a font,
    /// a bitmap placed on its own, a button, a text or a video.
    NotDrawn {
        /// The tag code of the definition.
        code: u16,
    },
    /// Sprites are nested deeper than [`MAX_SPRITE_DEPTH`].
    SpriteDepth,
    /// The frame holds more than [`MAX_SCENE_PLACEMENTS`] placements.
    TooManyPlacements,
    /// The frame draws more than [`MAX_SCENE_PATHS`] paths or [`MAX_SCENE_SEGMENTS`] path
    /// commands.
    TooManyPaths,
    /// Converting shapes and morph shapes would read more than [`MAX_SCENE_RECORDS`] records.
    TooManyRecords,
    /// The fills of the frame would cover more than [`MAX_COVERAGE_FACTOR`] times its canvas;
    /// this and every later element are not drawn.
    FillBudget,
    /// The frame holds more than [`MAX_SCENE_MASKS`] masks.
    TooManyMasks,
    /// A mask inside a mask, which is skipped; the depths it covers are clipped by the outer
    /// mask only.
    NestedMask,
    /// A gradient has more than [`MAX_GRADIENT_STOPS`] stops; the layer is skipped.
    GradientStops {
        /// The stops of the gradient.
        count: usize,
    },
    /// A bitmap fill names a bitmap that could not be decoded; the layer is skipped.
    MissingBitmap {
        /// The ID of the bitmap.
        bitmap_id: u16,
    },
    /// A paint or a path cannot be drawn, e.g. for a transform that cannot be inverted.
    InvalidGeometry,
    /// The rasteriser failed on a layer; the layer is skipped.
    Raster,
    /// The masks of the frame need more memory than allowed; the clipped layer is skipped.
    MaskMemory,
    /// The bitmaps with colour transforms need more memory than allowed; the layer is skipped.
    BitmapMemory,
    /// The frame lays out more than [`MAX_SCENE_GLYPHS`] characters of text; this and every later
    /// element are not drawn.
    TooManyGlyphs,
    /// The font of a text is not in the movie or is stripped of its glyph shapes; its characters
    /// are drawn as empty boxes. `id` is `65535` for a font named only by its class.
    MissingFont {
        /// The ID of the font the text names.
        id: u16,
    },
    /// The font of a text has no glyph for a character, which is drawn as an empty box.
    MissingGlyph {
        /// The UTF-16 code unit.
        code: u16,
    },
    /// A static text names a glyph beyond the glyph table of its font, drawn as an empty box.
    MissingGlyphIndex {
        /// The index in the glyph table.
        index: u16,
    },
    /// A localisation key of a text is not in the texts; the key is drawn as it is.
    Unlocalised,
    /// A text is longer than [`MAX_TEXT_CHARS`] characters; the rest is not drawn.
    TextTruncated,
    /// The placement has filters, which are not applied.
    Filters,
    /// The placement has a blend mode other than normal, which is not applied.
    BlendMode {
        /// The blend mode.
        mode: u8,
    },
}

impl SkipReason {
    /// Returns a short name of the reason, in snake case.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownDefinition => "unknown_definition",
            Self::NotDrawn { .. } => "not_drawn",
            Self::SpriteDepth => "sprite_depth",
            Self::TooManyPlacements => "too_many_placements",
            Self::TooManyPaths => "too_many_paths",
            Self::TooManyRecords => "too_many_records",
            Self::FillBudget => "fill_budget",
            Self::TooManyMasks => "too_many_masks",
            Self::NestedMask => "nested_mask",
            Self::GradientStops { .. } => "gradient_stops",
            Self::MissingBitmap { .. } => "missing_bitmap",
            Self::InvalidGeometry => "invalid_geometry",
            Self::Raster => "raster",
            Self::MaskMemory => "mask_memory",
            Self::BitmapMemory => "bitmap_memory",
            Self::TooManyGlyphs => "too_many_glyphs",
            Self::MissingFont { .. } => "missing_font",
            Self::MissingGlyph { .. } => "missing_glyph",
            Self::MissingGlyphIndex { .. } => "missing_glyph_index",
            Self::Unlocalised => "unlocalised",
            Self::TextTruncated => "text_truncated",
            Self::Filters => "filters",
            Self::BlendMode { .. } => "blend_mode",
        }
    }

    /// Returns whether the element was drawn, only without the effect.
    #[must_use]
    pub const fn is_effect(&self) -> bool {
        matches!(
            self,
            Self::Filters
                | Self::BlendMode { .. }
                | Self::MissingFont { .. }
                | Self::MissingGlyph { .. }
                | Self::MissingGlyphIndex { .. }
                | Self::Unlocalised
                | Self::TextTruncated
        )
    }
}

impl fmt::Display for SkipReason {
    #[expect(clippy::too_many_lines, reason = "one arm per reason")]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDefinition => formatter.write_str("no definition has this ID"),
            Self::NotDrawn { code } => match tag_name(*code) {
                Some(name) => write!(formatter, "{name} definitions are not drawn"),
                None => write!(formatter, "definitions of tag {code} are not drawn"),
            },
            Self::SpriteDepth => write!(
                formatter,
                "sprites are nested deeper than {MAX_SPRITE_DEPTH} levels"
            ),
            Self::TooManyPlacements => write!(
                formatter,
                "the frame holds more than {MAX_SCENE_PLACEMENTS} placements"
            ),
            Self::TooManyPaths => write!(
                formatter,
                "the frame draws more than {MAX_SCENE_PATHS} paths or {MAX_SCENE_SEGMENTS} path commands"
            ),
            Self::TooManyRecords => write!(
                formatter,
                "converting the shapes would read more than {MAX_SCENE_RECORDS} records"
            ),
            Self::FillBudget => write!(
                formatter,
                "the fills would cover more than {MAX_COVERAGE_FACTOR} times the canvas; the rest is not drawn"
            ),
            Self::TooManyMasks => write!(
                formatter,
                "the frame holds more than {MAX_SCENE_MASKS} masks"
            ),
            Self::NestedMask => formatter.write_str("a mask inside a mask is skipped"),
            Self::GradientStops { count } => write!(
                formatter,
                "a gradient has {count} stops, more than {MAX_GRADIENT_STOPS}"
            ),
            Self::MissingBitmap { bitmap_id } => {
                write!(formatter, "bitmap {bitmap_id} could not be decoded")
            }
            Self::InvalidGeometry => {
                formatter.write_str("a path or paint has a transform that cannot be drawn")
            }
            Self::Raster => formatter.write_str("the rasteriser failed on a path"),
            Self::MaskMemory => formatter.write_str("the masks need more memory than allowed"),
            Self::BitmapMemory => {
                formatter.write_str("the colour-transformed bitmaps need more memory than allowed")
            }
            Self::TooManyGlyphs => write!(
                formatter,
                "the frame lays out more than {MAX_SCENE_GLYPHS} characters of text; the rest is not drawn"
            ),
            Self::MissingFont { id } if *id == u16::MAX => {
                formatter.write_str("the font of a text is not in the movie; empty boxes are drawn")
            }
            Self::MissingFont { id } => write!(
                formatter,
                "font {id} is not in the movie or has no glyph shapes; empty boxes are drawn"
            ),
            Self::MissingGlyph { code } => write!(
                formatter,
                "the font has no glyph for U+{code:04X}; an empty box is drawn"
            ),
            Self::MissingGlyphIndex { index } => write!(
                formatter,
                "the font has no glyph {index}; an empty box is drawn"
            ),
            Self::Unlocalised => {
                formatter.write_str("a localisation key is unknown; the key is drawn")
            }
            Self::TextTruncated => write!(
                formatter,
                "a text is longer than {MAX_TEXT_CHARS} characters; the rest is not drawn"
            ),
            Self::Filters => formatter.write_str("filters are not applied"),
            Self::BlendMode { mode } => write!(formatter, "blend mode {mode} is not applied"),
        }
    }
}
