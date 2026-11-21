// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::RasterOptions;

use super::ui_render_frame_evidence::BackendInputs;
use super::ui_render_glyph_sources::{UiRenderedFontMetrics, UiRenderedGlyph};
use super::ui_render_localization::UiRenderedLocalization;
use super::ui_render_summary::RenderSummaryOrigins;
use crate::output_file::read_error;
use crate::{ConversionError, FieldOrigin, SourceInventory, UiMovie, UiRender};

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
    /// Aggregate origin of the encoded PNG payload.
    pub artifact_origin: FieldOrigin,
    pub(crate) summary: RenderSummaryOrigins,
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
                &BackendInputs {
                    items: &raster.item_inputs,
                    bitmaps: &raster.bitmap_inputs,
                    masks: &[],
                    title: false,
                    diagnostics_complete: raster.raster.skipped.is_empty()
                        && frame.has_no_omissions(),
                },
                options,
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
            artifact_origin: evidence.artifact_origin,
            summary: evidence.summary,
        })
    }
}
