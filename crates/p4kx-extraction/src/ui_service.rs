// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "ui_bitmap_decode.rs"]
pub(crate) mod ui_bitmap_decode;
#[path = "ui_bitmap_sources.rs"]
pub(crate) mod ui_bitmap_sources;
#[path = "ui_extract.rs"]
pub(crate) mod ui_extract;
#[path = "ui_extract_diagnostics.rs"]
mod ui_extract_diagnostics;
#[path = "ui_extract_evidence.rs"]
pub(crate) mod ui_extract_evidence;
#[path = "ui_font_sources.rs"]
pub(crate) mod ui_font_sources;
#[path = "ui_jobs.rs"]
pub(crate) mod ui_jobs;
#[path = "ui_json_fields.rs"]
pub(crate) mod ui_json_fields;
#[path = "ui_movie.rs"]
pub(crate) mod ui_movie;
#[path = "ui_movie_evidence.rs"]
pub(crate) mod ui_movie_evidence;
#[path = "ui_render_bitmap_sources.rs"]
mod ui_render_bitmap_sources;
#[path = "ui_render_frame_evidence.rs"]
mod ui_render_frame_evidence;
#[path = "ui_render_geometry.rs"]
mod ui_render_geometry;
#[path = "ui_render_glyph_sources.rs"]
mod ui_render_glyph_sources;
#[path = "ui_render_localization.rs"]
mod ui_render_localization;
#[path = "ui_render_publication.rs"]
mod ui_render_publication;
#[path = "ui_render_summary.rs"]
mod ui_render_summary;

pub use ui_extract::{
    ObservedUiExtraction, UiAssetFailure, UiAssetKind, UiExtraction, WrittenUiAsset,
};
pub use ui_movie::{MAX_UI_MOVIE_LEN, UiMovie};
pub use ui_movie_evidence::{UiMovieEvidence, UiSourceSite};
pub use ui_render_bitmap_sources::{UiRenderBitmapSources, UiRenderedBitmap};
pub use ui_render_glyph_sources::{UiRenderedFontMetrics, UiRenderedGlyph};
pub use ui_render_localization::UiRenderedLocalization;
pub use ui_render_publication::PublishedUiRender;
pub use ui_render_summary::{RenderSummaryOrigins, UnresolvedImportOrigins};
