// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_gfx::{RasterOptions, Skipped, TagFailure};

use crate::ui_imports::ImportReport;
use crate::{ConversionError, UiBitmapFailure, UiMovie};

/// What [`UiMovie::render_svg_to_file`] wrote: the size of the document and what it leaves out.
#[derive(Debug)]
pub struct UiSvg {
    /// The width of the document in pixels.
    pub width: u32,
    /// The height of the document in pixels.
    pub height: u32,
    /// The elements skipped or drawn without an effect, in drawing order.
    pub skipped: Vec<Skipped>,
    /// The bitmaps that could not be decoded or kept, ascending by ID; their fills are skipped.
    pub bitmap_failures: Vec<UiBitmapFailure>,
    /// The tags of the movie that could not be read, whose definitions are therefore missing.
    pub tag_failures: Vec<TagFailure>,
    /// The movies read to resolve imported fonts and the fonts that were not found.
    pub imports: ImportReport,
}

impl UiSvg {
    /// Returns whether every tag was read and every element written with all its effects.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.bitmap_failures.is_empty() && self.tag_failures.is_empty()
    }
}

impl UiMovie<'_> {
    /// Renders the first frame as [`UiMovie::render`] does, but writes it to `out` as an SVG
    /// document, replacing the file.
    ///
    /// The frame is composed, its bitmaps decoded and the budgets charged exactly as for the
    /// raster, so both end at the same item. The document is streamed; bitmaps are embedded as
    /// PNG data URIs, never referenced by path, and the title, the Entry path or the file name
    /// without its directory, is escaped. A sibling origin companion binds the committed SVG.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when `out` is the movie file, the errors of
    /// [`UiMovie::render`], and [`ConversionError::Write`] when the file cannot be written.
    pub fn render_svg_to_file(
        &self,
        options: &RasterOptions,
        out: &Path,
    ) -> Result<UiSvg, ConversionError> {
        self.render_svg_to_file_with_origin(options, out)
            .map(|written| written.render)
    }
}
