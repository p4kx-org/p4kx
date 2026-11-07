// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::ColorImage;
use p4kx_extraction::gfx::RasterOptions;
use p4kx_extraction::{CancelToken, LocalizationEntry, ScreenTexts, UiMovie, WriteProtectionError};
use p4kx_screens::{StillFormat, StillRequest, Variable};

use super::contents::UiContents;
use super::report::UiReport;
use super::settings::{UiBackground, UiSize};
use super::still;
use crate::forge_cache::ReadFailure;
use crate::model::describe;
use crate::{EntryIndex, TextLanguage};

/// What the tab draws: a GFx or SWF movie of the Archive, or a `BuildingBlocks` screen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiSource {
    /// The movie Entry at this path.
    Movie(String),
    /// The canvas at this record path, drawn in the colours of the brand.
    Screen {
        /// The record path of the canvas.
        canvas: String,
        /// The manufacturer code of the ship, when the Archive has a style for it.
        brand: Option<String>,
    },
}

/// Everything a picture depends on; a new key draws a new picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RenderKey {
    pub(super) source: UiSource,
    pub(super) size: UiSize,
    pub(super) background: UiBackground,
    pub(super) language: Option<TextLanguage>,
    pub(super) variables: Vec<Variable>,
}

impl RenderKey {
    pub(super) const fn raster(&self) -> RasterOptions {
        RasterOptions {
            size: self.size.canvas(),
            background: self.background.background(),
        }
    }
}

/// A picture the tab drew and what it left out.
#[derive(Debug)]
pub struct UiRendered {
    pub(super) serial: u64,
    pub(super) source: UiSource,
    pub(super) image: Arc<ColorImage>,
    pub(super) report: UiReport,
    pub(super) contents: Option<UiContents>,
}

impl UiRendered {
    /// Returns a number that differs between pictures, to tell when to upload a new texture.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns what was drawn.
    #[must_use]
    pub const fn source(&self) -> &UiSource {
        &self.source
    }

    /// Returns the pixels.
    #[must_use]
    pub const fn image(&self) -> &Arc<ColorImage> {
        &self.image
    }

    /// Returns the width of the picture in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.image.size[0]
    }

    /// Returns the height of the picture in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.image.size[1]
    }

    /// Returns what the render left out.
    #[must_use]
    pub const fn report(&self) -> &UiReport {
        &self.report
    }

    /// Returns what a movie is made of; `None` for a screen, whose contents the listing of the
    /// ship gives.
    #[must_use]
    pub const fn contents(&self) -> Option<&UiContents> {
        self.contents.as_ref()
    }
}

/// A picture drawn on the background thread.
#[derive(Debug)]
pub(super) struct Built {
    pub(super) image: ColorImage,
    pub(super) report: UiReport,
    pub(super) contents: Option<UiContents>,
    pub(super) texts: Option<Arc<ScreenTexts>>,
}

/// Why a picture could not be drawn.
#[derive(Debug)]
pub(super) enum Fail {
    ArchiveChanged,
    Stopped,
    Reason(String),
    Protected(WriteProtectionError),
}

/// Fails with [`Fail::Stopped`] once `cancel` is cancelled.
pub(super) fn checkpoint(cancel: &CancelToken) -> Result<(), Fail> {
    if cancel.is_cancelled() {
        Err(Fail::Stopped)
    } else {
        Ok(())
    }
}

impl From<ReadFailure> for Fail {
    fn from(failure: ReadFailure) -> Self {
        match failure {
            ReadFailure::ArchiveChanged(_) => Self::ArchiveChanged,
            ReadFailure::Unreadable(reason) => Self::Reason(reason),
        }
    }
}

pub(super) fn reason(error: &(dyn std::error::Error + 'static)) -> Fail {
    match crate::installation::refusal_in(error) {
        Some(refused) if refused.is_cancelled() => Fail::Stopped,
        Some(refused) => Fail::Protected(refused.clone()),
        None => Fail::Reason(describe(error)),
    }
}

/// Draws the picture of `key` as the PNG of `p4kx ui render` would hold it.
pub(super) fn draw(
    index: &EntryIndex,
    key: &RenderKey,
    cached: Option<Arc<ScreenTexts>>,
    cancel: &CancelToken,
) -> Result<Built, Fail> {
    checkpoint(cancel)?;
    if index.watch().check().is_err() {
        return Err(Fail::ArchiveChanged);
    }
    let built = match &key.source {
        UiSource::Movie(path) => movie(index, path, (key, cancel))?,
        UiSource::Screen { canvas, brand } => {
            screen(index, (canvas, brand.as_deref()), (key, cancel), cached)?
        }
    };
    checkpoint(cancel)?;
    if index.watch().check().is_err() {
        return Err(Fail::ArchiveChanged);
    }
    Ok(built)
}

fn pixels(image: &p4kx_extraction::gfx::RgbaImage) -> ColorImage {
    let extent = |value: u32| usize::try_from(value).unwrap_or(0);
    let size = [extent(image.width()), extent(image.height())];
    ColorImage::from_rgba_unmultiplied(size, image.pixels())
}

/// Opens the movie at `path` with the texts of the language of `key`, if the Archive has them.
pub(super) fn open_movie<'a>(
    index: &'a EntryIndex,
    path: &str,
    language: Option<TextLanguage>,
) -> Result<(UiMovie<'a>, Vec<String>), Fail> {
    let archive = index.entries_archive().map_err(|e| reason(&e))?;
    let movie = UiMovie::from_archive(archive, path).map_err(|e| reason(&e))?;
    let Some(language) = language else {
        return Ok((movie, Vec::new()));
    };
    Ok(match LocalizationEntry::read(archive, language) {
        Ok(texts) => (movie.with_texts(texts), Vec::new()),
        Err(error) => (movie, vec![error.to_string()]),
    })
}

fn movie(
    index: &EntryIndex,
    path: &str,
    (key, cancel): (&RenderKey, &CancelToken),
) -> Result<Built, Fail> {
    let (movie, warnings) = open_movie(index, path, key.language)?;
    checkpoint(cancel)?;
    let render = movie.render(&key.raster()).map_err(|e| reason(&e))?;
    checkpoint(cancel)?;
    let assets = movie.movie().assets();
    let contents = UiContents::of_movie(&movie, &assets);
    Ok(Built {
        image: pixels(&render.image),
        report: UiReport::of_movie(
            &render.skipped,
            &render.bitmap_failures,
            &render.tag_failures,
            &render.imports,
        )
        .warned(&warnings),
        contents: Some(contents),
        texts: None,
    })
}

fn screen(
    index: &EntryIndex,
    (canvas, brand): (&str, Option<&str>),
    (key, cancel): (&RenderKey, &CancelToken),
    cached: Option<Arc<ScreenTexts>>,
) -> Result<Built, Fail> {
    let source = index.forge().read(index, || {})?;
    checkpoint(cancel)?;
    let forge = source.entry.open().map_err(|e| reason(&e))?;
    let texts = match cached {
        Some(texts) => texts,
        None => Arc::new(still::load_texts(index, &forge, key.language)?),
    };
    checkpoint(cancel)?;
    let request = StillRequest {
        screen: canvas,
        brand,
        variables: &key.variables,
        raster: key.raster(),
        format: StillFormat::Png,
    };
    let (still, warnings) = still::draw(index, &forge, &texts, &request, cancel)?;
    let image = still
        .image()
        .ok_or_else(|| Fail::Reason("no pixels".to_owned()))?;
    Ok(Built {
        image: pixels(image),
        report: UiReport::of_still(&still, &warnings),
        contents: None,
        texts: Some(texts),
    })
}
