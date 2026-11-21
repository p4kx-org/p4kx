// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_gfx::{
    Assets, GfxError, Localise, MAX_BITMAP_PIXELS, Raster, RasterOptions, RgbaImage, Scene,
    SceneOrigins, Skipped, TagFailure, TextOptions,
};
use p4kx_localization::decode_line_breaks;

use crate::texture_parts::is_same_file;
use crate::ui_imports::{ImportReport, ImportReportSources, ImportedFonts, resolve_fonts};
use crate::ui_movie::Origin;
use crate::ui_render_bitmaps::FrameBitmaps;
use crate::ui_service::ui_font_sources::FrameFonts;
use crate::{ConversionError, UiMovie};

/// The pixels the decoded bitmaps of one rendered frame may hold together, granted in ascending
/// ID order, and the pixels its parallel bitmap decodes may hold at the same time.
pub const MAX_UI_RENDER_BITMAP_PIXELS: u64 = 2 * MAX_BITMAP_PIXELS;

/// A bitmap a rendered frame fills with that could not be decoded.
#[derive(Debug)]
pub struct UiBitmapFailure {
    /// The ID of the bitmap.
    pub id: u16,
    /// Why.
    pub error: ConversionError,
}

/// What [`UiMovie::render`] drew: the image and what it left out.
#[derive(Debug)]
pub struct UiRender {
    /// The image with straight alpha.
    pub image: RgbaImage,
    /// The elements skipped or drawn without an effect, in drawing order.
    pub skipped: Vec<Skipped>,
    /// The bitmaps that could not be decoded or kept, ascending by ID; their fills are skipped.
    pub bitmap_failures: Vec<UiBitmapFailure>,
    /// The tags of the movie that could not be read, whose definitions are therefore missing.
    pub tag_failures: Vec<TagFailure>,
    /// The movies read to resolve imported fonts and the fonts that were not found.
    pub imports: ImportReport,
}

/// A composed first frame with its decoded bitmaps.
pub(crate) struct ComposedFrame<'archive> {
    pub(crate) scene: Scene,
    pub(crate) origins: Option<SceneOrigins>,
    pub(crate) fonts: FrameFonts<'archive>,
    pub(crate) import_sources: ImportReportSources<'archive>,
    pub(crate) bitmaps: FrameBitmaps<'archive>,
    pub(crate) tag_failures: Vec<TagFailure>,
    pub(crate) imports: ImportReport,
}

impl UiRender {
    /// Returns whether every tag was read and every element drawn with all its effects.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.bitmap_failures.is_empty() && self.tag_failures.is_empty()
    }
}

impl<'archive> UiMovie<'archive> {
    /// Renders the first frame of the main timeline, with its sprites, to an image.
    ///
    /// The bitmaps the frame fills with are decoded in parallel as [`UiMovie::extract_assets`]
    /// decodes them, external images and sub-images included, each cut bitmap once, and hold at
    /// most [`MAX_UI_RENDER_BITMAP_PIXELS`] together. Elements that cannot be drawn are skipped and
    /// listed in the result.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UiMovie`] when the stage has no area and
    /// [`ConversionError::UiCanvas`] when the canvas is empty or too large, and
    /// [`ConversionError::UiTexts`] when the texts given to the movie cannot be indexed.
    pub fn render(&self, options: &RasterOptions) -> Result<UiRender, ConversionError> {
        let frame = self.compose_frame(options)?;
        let raster = frame
            .scene
            .rasterize(&frame.bitmaps.images, options)
            .map_err(|source| self.frame_error(source))?;
        Ok(frame.into_render(raster))
    }

    /// Composes the first frame and decodes the bitmaps it fills with, after checking that the
    /// canvas for `options` can be drawn.
    pub(crate) fn compose_frame(
        &self,
        options: &RasterOptions,
    ) -> Result<ComposedFrame<'archive>, ConversionError> {
        self.compose_frame_with_assets(options, self.movie().assets(), false)
    }

    pub(crate) fn compose_frame_with_assets(
        &self,
        options: &RasterOptions,
        mut assets: Assets<'_>,
        observe: bool,
    ) -> Result<ComposedFrame<'archive>, ConversionError> {
        let ini = self
            .texts()
            .map(|texts| {
                texts.parse().map_err(|source| ConversionError::UiTexts {
                    path: texts.path().to_owned(),
                    source,
                })
            })
            .transpose()?;
        let lookup = |key: &str| {
            ini.as_ref()?
                .resolve(key)
                .map(|text| decode_line_breaks(text).into_owned())
        };
        let imported = match self.origin() {
            Origin::Entry(archive) => {
                resolve_fonts(archive, self.name(), &assets, self.font_language(), observe)
            }
            Origin::File(_) => ImportedFonts::default(),
        };
        let text_options = TextOptions {
            fonts: &imported.fonts,
            localise: ini.as_ref().map(|_| &lookup as Localise<'_>),
        };
        let (scene, origins) = if observe {
            let observed = self.movie().first_frame_observed(&assets, &text_options);
            (observed.scene, Some(observed.origins))
        } else {
            (
                self.movie().first_frame_with_text(&assets, &text_options),
                None,
            )
        };
        scene
            .canvas_size(options.size)
            .map_err(|source| self.frame_error(source))?;
        let bitmaps = self.frame_bitmaps(&assets, &scene.bitmap_ids());
        Ok(ComposedFrame {
            scene,
            origins,
            fonts: FrameFonts::new(assets.fonts.len(), imported.sources),
            bitmaps,
            tag_failures: std::mem::take(&mut assets.failures),
            imports: imported.report,
            import_sources: imported.report_sources,
        })
    }

    /// Refuses an output file that is the movie file itself.
    pub(crate) fn check_output(&self, out: &Path) -> Result<(), ConversionError> {
        if let Origin::File(file) = self.origin()
            && is_same_file(out, [file.clone()])
        {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Wraps the error of drawing the frame.
    pub(crate) fn frame_error(&self, source: GfxError) -> ConversionError {
        match source {
            GfxError::CanvasSize { .. } => ConversionError::UiCanvas {
                movie: self.name().to_owned(),
                source,
            },
            source => ConversionError::UiMovie {
                movie: self.name().to_owned(),
                source,
            },
        }
    }

    /// Renders the first frame as [`UiMovie::render`] does and writes it to `out` as an 8-bit
    /// RGBA PNG, replacing the file.
    ///
    /// The committed PNG is bound by a sibling source-origin companion.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when `out` is the movie file, the errors of
    /// [`UiMovie::render`], and [`ConversionError::Write`] when the file cannot be written.
    pub fn render_to_file(
        &self,
        options: &RasterOptions,
        out: &Path,
    ) -> Result<UiRender, ConversionError> {
        self.render_to_file_with_origin(options, out)
            .map(|written| written.render)
    }
}

impl ComposedFrame<'_> {
    pub(crate) const fn has_no_omissions(&self) -> bool {
        self.bitmaps.failures.is_empty()
            && self.tag_failures.is_empty()
            && self.imports.unresolved.is_empty()
    }

    pub(crate) fn into_render(self, raster: Raster) -> UiRender {
        UiRender {
            image: raster.image,
            skipped: raster.skipped,
            bitmap_failures: self.bitmaps.failures,
            tag_failures: self.tag_failures,
            imports: self.imports,
        }
    }
}
