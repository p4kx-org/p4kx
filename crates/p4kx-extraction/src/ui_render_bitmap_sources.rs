// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_gfx::RasterOptions;

use super::{ui_bitmap_sources, ui_render_geometry};
use crate::output_file::read_error;
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

/// A movie raster and the exact bitmap origins supplied to its paint operations.
#[derive(Debug)]
#[non_exhaustive]
pub struct UiRenderBitmapSources {
    /// The ordinary raster and diagnostics.
    pub render: UiRender,
    /// The bitmap inputs selected by the raster backend.
    pub bitmaps: Vec<UiRenderedBitmap>,
    /// Stage and drawing instructions supplied by the main movie.
    pub movie_geometry: FieldOrigin,
    /// The physical sources and uses referenced by the movie geometry and bitmap inputs.
    pub provenance: SourceInventory,
}

impl UiMovie<'_> {
    /// Renders the first frame and binds the bitmaps actually used by the raster backend.
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
        let evidence = || -> io::Result<_> {
            let uses = SourceUses::new();
            let movie = UiMovieEvidence::observe(self, &uses).map_err(io::Error::other)?;
            let movie_geometry = ui_render_geometry::register(
                &uses,
                &movie,
                &observed.origins,
                &frame,
                &raster.item_inputs,
                options.background,
            )?;
            let mut bitmaps = Vec::new();
            for id in raster.bitmap_inputs {
                let (order, source) = frame.bitmaps.source(id).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "drawn bitmap has no decoded source",
                    )
                })?;
                let (contributions, complete) = ui_bitmap_sources::contributions(
                    &movie,
                    &observed.origins,
                    &uses,
                    order,
                    source,
                )?;
                bitmaps.push(UiRenderedBitmap {
                    id,
                    origin: uses.origin(&contributions, complete),
                });
            }
            let provenance = uses.freeze().map_err(io::Error::other)?;
            Ok((bitmaps, movie_geometry, provenance))
        };
        let (bitmaps, movie_geometry, provenance) =
            evidence().map_err(|source| read_error(self.name().as_ref(), source))?;
        Ok(UiRenderBitmapSources {
            render: frame.into_render(raster.raster),
            bitmaps,
            movie_geometry,
            provenance,
        })
    }
}
