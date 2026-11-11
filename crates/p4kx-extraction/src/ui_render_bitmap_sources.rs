// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_gfx::{AssetOrigins, Background, RasterOptions};

use super::ui_render_glyph_sources::{self, UiRenderedFontMetrics, UiRenderedGlyph};
use super::ui_render_localization::{self, UiRenderedLocalization};
use super::{ui_bitmap_sources, ui_render_geometry};
use crate::output_file::read_error;
use crate::ui_render::ComposedFrame;
use crate::{
    ConversionError, FieldOrigin, SourceInventory, SourceUses, UiMovie, UiMovieEvidence, UiRender,
};

/// One bitmap supplied to an actual on-canvas paint operation.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderedBitmap {
    /// The bitmap ID used by the renderer.
    pub id: u16,
    /// The physical bitmap definition, its base and its actual decoded inputs.
    pub origin: FieldOrigin,
}

/// A movie raster and the origins supplied to its paint operations.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderBitmapSources {
    /// The ordinary raster and diagnostics.
    pub render: UiRender,
    /// The bitmap inputs selected by the raster backend.
    pub bitmaps: Vec<UiRenderedBitmap>,
    /// Stage and drawing instructions supplied by the main movie.
    pub movie_geometry: FieldOrigin,
    /// Actual font glyph outlines supplied to the rendered frame.
    pub glyphs: Vec<UiRenderedGlyph>,
    /// Font measurements used to position the drawn text.
    pub font_metrics: Vec<UiRenderedFontMetrics>,
    /// The resolved localisation values that supplied drawn text.
    pub localizations: Vec<UiRenderedLocalization>,
    /// The physical sources and uses referenced by the drawing origins.
    pub provenance: SourceInventory,
}

impl UiMovie<'_> {
    /// Renders the first frame and binds the observed drawing inputs.
    ///
    /// # Errors
    ///
    /// Returns render or source-evidence errors.
    pub fn render_with_bitmap_sources(
        &self,
        options: &RasterOptions,
    ) -> Result<UiRenderBitmapSources, ConversionError> {
        let observed = self.movie().assets_observed();
        let frame = self.compose_frame_with_assets(options, observed.assets, true)?;
        let raster = frame
            .scene
            .rasterize_observed(&frame.bitmaps.images, options)
            .map_err(|source| self.frame_error(source))?;
        let evidence = self
            .register_frame_evidence(
                &observed.origins,
                &frame,
                &raster.item_inputs,
                &raster.bitmap_inputs,
                options.background,
            )
            .map_err(|source| read_error(self.name().as_ref(), source))?;
        Ok(UiRenderBitmapSources {
            render: frame.into_render(raster.raster),
            bitmaps: evidence.bitmaps,
            movie_geometry: evidence.movie_geometry,
            glyphs: evidence.glyphs,
            font_metrics: evidence.font_metrics,
            localizations: evidence.localizations,
            provenance: evidence.provenance,
        })
    }

    fn register_frame_evidence(
        &self,
        origins: &AssetOrigins,
        frame: &ComposedFrame<'_>,
        item_inputs: &[usize],
        bitmap_inputs: &[u16],
        background: Background,
    ) -> io::Result<FrameEvidence> {
        let uses = SourceUses::new();
        let movie = UiMovieEvidence::observe(self, &uses).map_err(io::Error::other)?;
        let movie_geometry =
            ui_render_geometry::register(&uses, &movie, origins, frame, item_inputs, background)?;
        let text = ui_render_glyph_sources::register(&uses, &movie, origins, frame, item_inputs)?;
        let localizations = ui_render_localization::register(
            &uses,
            &movie,
            origins,
            self.texts(),
            text.localised_definitions,
        )?;
        let mut bitmaps = Vec::new();
        for &id in bitmap_inputs {
            let (order, source) = frame.bitmaps.source(id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "drawn bitmap has no decoded source",
                )
            })?;
            let (contributions, complete) =
                ui_bitmap_sources::contributions(&movie, origins, &uses, order, source)?;
            bitmaps.push(UiRenderedBitmap {
                id,
                origin: uses.origin(&contributions, complete),
            });
        }
        let provenance = uses.freeze().map_err(io::Error::other)?;
        Ok(FrameEvidence {
            bitmaps,
            movie_geometry,
            glyphs: text.glyphs,
            font_metrics: text.metrics,
            localizations,
            provenance,
        })
    }
}

struct FrameEvidence {
    bitmaps: Vec<UiRenderedBitmap>,
    movie_geometry: FieldOrigin,
    glyphs: Vec<UiRenderedGlyph>,
    font_metrics: Vec<UiRenderedFontMetrics>,
    localizations: Vec<UiRenderedLocalization>,
    provenance: SourceInventory,
}
