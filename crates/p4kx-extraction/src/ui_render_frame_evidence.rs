// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_gfx::{AssetOrigins, RasterOptions};

use super::ui_render_glyph_sources::{self, UiRenderedFontMetrics, UiRenderedGlyph};
use super::ui_render_localization::{self, UiRenderedLocalization};
use super::ui_render_summary::RenderSummaryOrigins;
use super::{ui_bitmap_sources, ui_render_geometry};
use crate::ui_render::ComposedFrame;
use crate::{
    FieldOrigin, SourceInventory, SourceUse, SourceUseRef, SourceUses, UiMovie, UiMovieEvidence,
    UiRenderedBitmap,
};

pub(crate) struct BackendInputs<'a> {
    pub(crate) items: &'a [usize],
    pub(crate) bitmaps: &'a [u16],
    pub(crate) masks: &'a [usize],
    pub(crate) title: bool,
    pub(crate) diagnostics_complete: bool,
}

pub(crate) struct FrameContributions {
    registry: SourceUses,
    artifact: BTreeMap<SourceUseRef, SourceUse>,
    complete: bool,
}

impl FrameContributions {
    fn new(complete: bool) -> Self {
        Self {
            registry: SourceUses::new(),
            artifact: BTreeMap::new(),
            complete,
        }
    }

    pub(crate) const fn registry(&self) -> &SourceUses {
        &self.registry
    }

    pub(crate) fn add(&mut self, actual: Vec<SourceUse>, complete: bool) -> FieldOrigin {
        let origin = self.registry.origin(&actual, complete);
        self.complete &= origin.source_set_complete();
        self.artifact.extend(
            actual
                .into_iter()
                .map(|usage| (usage.reference().clone(), usage)),
        );
        origin
    }

    fn finish(self) -> io::Result<(FieldOrigin, SourceInventory)> {
        let actual: Vec<_> = self.artifact.into_values().collect();
        let origin = self.registry.origin(&actual, self.complete);
        let inventory = self.registry.freeze().map_err(io::Error::other)?;
        Ok((origin, inventory))
    }
}

pub(crate) struct FrameEvidence {
    pub(crate) bitmaps: Vec<UiRenderedBitmap>,
    pub(crate) movie_geometry: FieldOrigin,
    pub(crate) glyphs: Vec<UiRenderedGlyph>,
    pub(crate) font_metrics: Vec<UiRenderedFontMetrics>,
    pub(crate) localizations: Vec<UiRenderedLocalization>,
    pub(crate) provenance: SourceInventory,
    pub(crate) artifact_origin: FieldOrigin,
    pub(crate) summary: RenderSummaryOrigins,
}

impl UiMovie<'_> {
    pub(crate) fn register_frame_evidence(
        &self,
        origins: &AssetOrigins,
        frame: &ComposedFrame<'_>,
        inputs: &BackendInputs<'_>,
        options: &RasterOptions,
    ) -> io::Result<FrameEvidence> {
        let mut contributions = FrameContributions::new(inputs.diagnostics_complete);
        let movie =
            UiMovieEvidence::observe(self, contributions.registry()).map_err(io::Error::other)?;
        let geometry = ui_render_geometry::register(
            &movie,
            origins,
            frame,
            inputs.items,
            inputs.masks,
            options.background,
        )?;
        let movie_geometry = contributions.add(geometry, movie.is_bound());
        let text = ui_render_glyph_sources::register(
            &mut contributions,
            &movie,
            origins,
            frame,
            inputs.items,
            inputs.masks,
        )?;
        let localizations = ui_render_localization::register(
            &mut contributions,
            &movie,
            origins,
            self.texts(),
            text.localised_definitions,
        )?;
        let bitmaps = bind_bitmaps(&mut contributions, &movie, origins, frame, inputs.bitmaps)?;
        let summary = RenderSummaryOrigins::register(
            &mut contributions,
            &movie,
            origins,
            frame,
            options.size,
            inputs.title,
        )?;
        let (artifact_origin, provenance) = contributions.finish()?;
        Ok(FrameEvidence {
            bitmaps,
            movie_geometry,
            glyphs: text.glyphs,
            font_metrics: text.metrics,
            localizations,
            provenance,
            artifact_origin,
            summary,
        })
    }
}

fn bind_bitmaps(
    contributions: &mut FrameContributions,
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    frame: &ComposedFrame<'_>,
    ids: &[u16],
) -> io::Result<Vec<UiRenderedBitmap>> {
    ids.iter()
        .map(|&id| {
            let (order, source) = frame.bitmaps.source(id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "drawn bitmap has no decoded source",
                )
            })?;
            let (evidence, complete) = ui_bitmap_sources::contributions(
                movie,
                origins,
                contributions.registry(),
                order,
                source,
            )?;
            Ok(UiRenderedBitmap {
                id,
                origin: contributions.add(evidence, complete),
            })
        })
        .collect()
}
