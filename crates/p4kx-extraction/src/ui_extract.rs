// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_gfx::{Assets, Bitmap, BitmapSource, GfxError, MAX_BITMAP_PIXELS, TagFailure};
use rayon::prelude::*;

use crate::budget::Budget;
use crate::output_file::write_error;
use crate::ui_jobs::{Job, Outcome, jobs};
use crate::{ConversionError, UiMovie};

/// The pixels the parallel bitmap jobs of one extraction may hold decoded at the same time.
const PIXEL_BUDGET: u64 = 2 * MAX_BITMAP_PIXELS;

/// The kind of a definition [`UiMovie::extract_assets`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiAssetKind {
    /// A bitmap, written as `bitmap_<id>.png`.
    Bitmap,
    /// A shape, written as `shape_<id>.json`.
    Shape,
    /// A morph shape, written as `morph_shape_<id>.json`.
    MorphShape,
}

impl UiAssetKind {
    /// Returns the name of the kind, which starts its file names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bitmap => "bitmap",
            Self::Shape => "shape",
            Self::MorphShape => "morph_shape",
        }
    }

    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Bitmap => "png",
            Self::Shape | Self::MorphShape => "json",
        }
    }
}

/// One definition [`UiMovie::extract_assets`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenUiAsset {
    /// The kind of the definition.
    pub kind: UiAssetKind,
    /// The ID of the definition.
    pub id: u16,
    /// The file written.
    pub path: PathBuf,
    /// Its size in bytes.
    pub size: u64,
}

/// One definition [`UiMovie::extract_assets`] could not write.
#[derive(Debug)]
pub struct UiAssetFailure {
    /// The kind of the definition.
    pub kind: UiAssetKind,
    /// The ID of the definition.
    pub id: u16,
    /// Why.
    pub error: ConversionError,
}

/// What [`UiMovie::extract_assets`] did: bitmaps, shapes and morph shapes, each in file order.
#[derive(Debug, Default)]
pub struct UiExtraction {
    /// The definitions written.
    pub written: Vec<WrittenUiAsset>,
    /// The definitions that failed.
    pub failures: Vec<UiAssetFailure>,
    /// The tags of the movie that could not be read, whose definitions are therefore missing.
    pub tag_failures: Vec<TagFailure>,
}

impl UiExtraction {
    /// Returns whether every tag was read and every definition written.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.tag_failures.is_empty()
    }
}

impl UiMovie<'_> {
    /// Writes every bitmap of the movie as an 8-bit RGBA PNG and every shape and morph shape as
    /// JSON in twips into `folder`, in parallel, replacing existing files; a definition that fails
    /// leaves no file and does not stop the others. Of several definitions of one kind with the
    /// same ID only the first is written.
    ///
    /// Embedded and lossless bitmaps are decoded from the movie; an external image is decoded
    /// from the DDS texture its file name names, relative to the movie, with the extension `.dds`
    /// tried as well; each bitmap that sub-images cut is decoded once for itself and its cuts.
    /// The bitmaps decoded at the same time hold at most twice [`MAX_BITMAP_PIXELS`] together.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Write`] when `folder` cannot be created.
    pub fn extract_assets(&self, folder: &Path) -> Result<UiExtraction, ConversionError> {
        crate::check_protected_output(folder, self.write_protection()?, None)?;
        fs::create_dir_all(folder).map_err(|source| write_error(folder, source))?;
        let mut assets = self.movie().assets();
        let budget = Budget::new(PIXEL_BUDGET);
        let mut outcomes: Vec<Outcome> = {
            let jobs = jobs(&assets);
            jobs.par_iter()
                .flat_map_iter(|job| self.run_job(&assets, job, folder, &budget))
                .collect()
        };
        outcomes.sort_by_key(|outcome| outcome.order);
        let mut extraction = UiExtraction::default();
        for outcome in outcomes {
            match outcome.result {
                Ok(written) => extraction.written.push(written),
                Err(error) => extraction.failures.push(UiAssetFailure {
                    kind: outcome.kind,
                    id: outcome.id,
                    error,
                }),
            }
        }
        extraction.tag_failures = std::mem::take(&mut assets.failures);
        Ok(extraction)
    }

    pub(crate) fn asset_error(&self, id: u16, source: GfxError) -> ConversionError {
        ConversionError::UiAsset {
            movie: self.name().to_owned(),
            id,
            source,
        }
    }

    fn run_job(
        &self,
        assets: &Assets<'_>,
        job: &Job<'_, '_>,
        folder: &Path,
        budget: &Budget,
    ) -> Vec<Outcome> {
        match job {
            Job::Shape(order, shape) => {
                vec![
                    self.write_file(*order, folder, UiAssetKind::Shape, shape.id, |out| {
                        shape.write_json(out)
                    }),
                ]
            }
            Job::MorphShape(order, morph) => {
                vec![
                    self.write_file(*order, folder, UiAssetKind::MorphShape, morph.id, |out| {
                        morph.write_json(out)
                    }),
                ]
            }
            Job::MissingBase(order, sub) => {
                vec![Outcome::bitmap(*order, sub.id, Err(self.cut_error(sub)))]
            }
            Job::Bitmaps { base, cuts } => self.write_bitmaps(assets, *base, cuts, folder, budget),
        }
    }

    pub(crate) fn cut_error(&self, sub: &Bitmap<'_>) -> ConversionError {
        let image_id = match &sub.source {
            BitmapSource::SubImage(cut) => cut.image_id,
            _ => sub.id,
        };
        ConversionError::UiSubImage {
            movie: self.name().to_owned(),
            id: sub.id,
            image_id,
        }
    }
}
