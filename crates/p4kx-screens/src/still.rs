// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_dataforge::DataForge;
use p4kx_gfx::{Raster, RasterOptions, RgbaImage, Skipped, SvgReport};

use crate::compose::{Composed, ScreenComposer, TextSources};
use crate::error::ScreenError;
use crate::images::ImageSources;
use crate::report::Report;
use crate::still_inputs::StillInputs;
use crate::variables::Variable;

/// The file format a [`Still`] is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StillFormat {
    /// An 8-bit RGBA PNG.
    Png,
    /// An SVG document.
    Svg,
}

/// What [`Still::render`] draws: one canvas, how, and in which colours.
#[derive(Debug, Clone, Copy)]
pub struct StillRequest<'a> {
    /// The canvas to draw, a GUID or a record path.
    pub screen: &'a str,
    /// The manufacturer code whose colour style the canvas is drawn with.
    pub brand: Option<&'a str>,
    /// The game variables the bindings are evaluated with instead of zero.
    pub variables: &'a [Variable],
    /// The size of the image and what lies under it.
    pub raster: RasterOptions,
    /// The format the still is made for.
    pub format: StillFormat,
}

#[derive(Debug)]
enum Picture {
    Png(RgbaImage),
    Svg(Vec<u8>),
}

/// A screen drawn as an image or an SVG document, with what the drawing left out.
#[derive(Debug)]
pub struct Still {
    picture: Picture,
    width: u32,
    height: u32,
    report: Report,
    skipped: Vec<Skipped>,
}

/// A rendered Screen with the source requests used by its composition and backend.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedStill {
    /// Pixels or SVG and the regular composition report.
    pub still: Still,
    /// Actual contributing inputs retained by the renderer.
    pub inputs: StillInputs,
}

impl Still {
    /// Composes the canvas of `request` of `forge` and draws it in the format of the request,
    /// the texts with `texts` and the textures and vector graphics with `images`.
    ///
    /// Widgets that need a source that is `None` are listed in the report.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`ScreenComposer::compose`], and [`ScreenError::Gfx`] when the size
    /// of the request cannot be drawn.
    pub fn render(
        forge: &DataForge<'_>,
        texts: Option<TextSources<'_>>,
        images: Option<ImageSources<'_>>,
        request: &StillRequest<'_>,
    ) -> Result<Self, ScreenError> {
        Self::render_while(forge, texts, images, request, &|| false)
    }

    /// Like [`Still::render`], asking `stopped` before the composition, after it and before the
    /// rasterisation.
    ///
    /// # Errors
    ///
    /// Returns [`ScreenError::Stopped`] when `stopped` returned `true`, and the errors of
    /// [`Still::render`].
    pub fn render_while(
        forge: &DataForge<'_>,
        texts: Option<TextSources<'_>>,
        images: Option<ImageSources<'_>>,
        request: &StillRequest<'_>,
        stopped: &dyn Fn() -> bool,
    ) -> Result<Self, ScreenError> {
        let scene = compose(ScreenComposer::new(forge), texts, images, request, stopped)?;
        let report = scene.report().clone();
        Ok(match request.format {
            StillFormat::Png => Self::png(scene.rasterize(&request.raster)?, report),
            StillFormat::Svg => {
                let mut bytes = Vec::new();
                let written = scene.write_svg(&request.raster, Some(request.screen), &mut bytes)?;
                Self::svg(bytes, written, report)
            }
        })
    }

    /// Renders a screen with its actual accepted texture, vector and text inputs.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::render`].
    pub fn render_observed<'a>(
        forge: &'a DataForge<'_>,
        texts: Option<TextSources<'a>>,
        images: Option<ImageSources<'a>>,
        request: &StillRequest<'_>,
        stopped: &dyn Fn() -> bool,
    ) -> Result<ObservedStill, ScreenError> {
        let composer = ScreenComposer::new(forge).observing_inputs();
        let mut scene = compose(composer, texts, images, request, stopped)?;
        let report = scene.report().clone();
        let (still, bitmap_ids, item_ordinals) = match request.format {
            StillFormat::Png => {
                let observed = scene.rasterize_observed(&request.raster)?;
                (
                    Self::png(observed.raster, report),
                    observed.bitmap_inputs,
                    observed.item_inputs,
                )
            }
            StillFormat::Svg => {
                let mut bytes = Vec::new();
                let observed =
                    scene.write_svg_observed(&request.raster, request.screen, &mut bytes)?;
                (
                    Self::svg(bytes, observed.svg, report),
                    observed.bitmap_inputs,
                    observed.item_inputs,
                )
            }
        };
        let inputs = scene.take_used_inputs(&bitmap_ids, item_ordinals);
        Ok(ObservedStill { still, inputs })
    }

    fn png(raster: Raster, report: Report) -> Self {
        Self {
            width: raster.image.width(),
            height: raster.image.height(),
            picture: Picture::Png(raster.image),
            skipped: raster.skipped,
            report,
        }
    }

    fn svg(bytes: Vec<u8>, output: SvgReport, report: Report) -> Self {
        Self {
            width: output.width,
            height: output.height,
            picture: Picture::Svg(bytes),
            skipped: output.skipped,
            report,
        }
    }

    /// Returns the format the still was drawn in.
    #[must_use]
    pub const fn format(&self) -> StillFormat {
        match self.picture {
            Picture::Png(_) => StillFormat::Png,
            Picture::Svg(_) => StillFormat::Svg,
        }
    }

    /// Returns the width of the image or document in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the image or document in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the pixels of a PNG still, with straight alpha; `None` for an SVG one.
    #[must_use]
    pub const fn image(&self) -> Option<&RgbaImage> {
        match &self.picture {
            Picture::Png(image) => Some(image),
            Picture::Svg(_) => None,
        }
    }

    /// Returns what the composition drew and left out.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// Returns the elements the renderer skipped or drew without an effect, in drawing order.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Returns whether the whole screen was drawn with every effect.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.report.is_complete() && self.skipped.is_empty()
    }

    /// Returns the bytes of the file: the PNG encoded, or the SVG document.
    ///
    /// # Errors
    ///
    /// Returns [`ScreenError::Gfx`] when the PNG cannot be encoded.
    pub fn encode(&self) -> Result<Cow<'_, [u8]>, ScreenError> {
        match &self.picture {
            Picture::Svg(bytes) => Ok(Cow::Borrowed(bytes)),
            Picture::Png(image) => {
                let mut bytes = Vec::new();
                image.write_png(&mut bytes)?;
                Ok(Cow::Owned(bytes))
            }
        }
    }
}

fn compose<'a>(
    mut composer: ScreenComposer<'a, '_>,
    texts: Option<TextSources<'a>>,
    images: Option<ImageSources<'a>>,
    request: &StillRequest<'_>,
    stopped: &dyn Fn() -> bool,
) -> Result<Composed, ScreenError> {
    if stopped() {
        return Err(ScreenError::Stopped);
    }
    composer = composer.with_variables(request.variables);
    if let Some(images) = images {
        composer = composer.with_images(images);
    }
    if let Some(texts) = texts {
        composer = composer.with_texts(texts);
    }
    let scene = composer.compose(request.screen, request.brand)?;
    if stopped() {
        return Err(ScreenError::Stopped);
    }
    Ok(scene)
}
