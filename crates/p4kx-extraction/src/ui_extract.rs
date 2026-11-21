// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_gfx::{Assets, Bitmap, BitmapSource, GfxError, MAX_BITMAP_PIXELS, TagFailure};
use rayon::prelude::*;

use super::{ui_extract_evidence, ui_json_fields};
use crate::budget::Budget;
use crate::output_file::write_error;
use crate::ui_jobs::{ArtifactFields, Job, JobResult, Outcome, UiJobOutput, jobs};
use crate::{
    ConversionError, DirectoryReceipt, DiskTextures, FieldOrigins, SourceInventory, UiMovie,
};

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

/// An extraction summary with immediate source origins and its directory companion.
#[derive(Debug)]
pub struct ObservedUiExtraction {
    pub(super) extraction: UiExtraction,
    pub(super) fields: FieldOrigins,
    pub(super) inventory: SourceInventory,
    pub(super) receipt: Option<PathBuf>,
}

impl ObservedUiExtraction {
    /// Returns the original extraction summary.
    #[must_use]
    pub const fn extraction(&self) -> &UiExtraction {
        &self.extraction
    }

    /// Returns the per-field origins of the structured extraction summary.
    #[must_use]
    pub const fn fields(&self) -> &FieldOrigins {
        &self.fields
    }

    /// Returns the actual original inputs of the summary and published artifacts.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    /// Returns the relative companion basename when this extraction committed artifacts.
    #[must_use]
    pub fn receipt(&self) -> Option<&Path> {
        self.receipt.as_deref()
    }
}

impl UiExtraction {
    /// Returns whether every tag was read and every definition written.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.tag_failures.is_empty()
    }
}

impl<'archive> UiMovie<'archive> {
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
    /// Returns output-directory or companion publication errors.
    pub fn extract_assets(&self, folder: &Path) -> Result<UiExtraction, ConversionError> {
        self.extract_assets_observed(folder)
            .map(|result| result.extraction)
    }

    /// Extracts the assets and returns their observed origin summary.
    ///
    /// # Errors
    ///
    /// Returns the errors of an asset extraction.
    pub fn extract_assets_observed(
        &self,
        folder: &Path,
    ) -> Result<ObservedUiExtraction, ConversionError> {
        let protection = self.write_protection()?;
        crate::check_protected_output(folder, protection, None)?;
        fs::create_dir_all(folder).map_err(|source| write_error(folder, source))?;
        let publication = DirectoryReceipt::new(folder, protection)
            .map_err(|source| write_error(folder, source))?;
        let output = UiJobOutput {
            folder,
            publication: &publication,
        };
        let observed = self.movie().assets_observed();
        let budget = Budget::new(PIXEL_BUDGET);
        let files = DiskTextures::default();
        let outcomes = jobs(&observed.assets)
            .par_iter()
            .map(|job| self.run_job(&observed.assets, job, &output, &budget, &files))
            .collect();
        ui_extract_evidence::complete(self, &publication, observed, outcomes)
    }

    pub(crate) fn asset_error(&self, id: u16, source: GfxError) -> ConversionError {
        ConversionError::UiAsset {
            movie: self.name().to_owned(),
            id,
            source,
        }
    }

    fn run_job<'owner>(
        &self,
        assets: &Assets<'_>,
        job: &Job<'_, '_>,
        output: &UiJobOutput<'owner, '_>,
        budget: &Budget,
        files: &DiskTextures,
    ) -> JobResult<'owner, 'archive> {
        match job {
            Job::Shape(order, shape) => JobResult::single(self.write_file(
                *order,
                output,
                UiAssetKind::Shape,
                shape.id,
                |out| {
                    ui_json_fields::write(*shape, out)
                        .map(ArtifactFields::Json)
                        .map_err(GfxError::Write)
                },
            )),
            Job::MorphShape(order, morph) => JobResult::single(self.write_file(
                *order,
                output,
                UiAssetKind::MorphShape,
                morph.id,
                |out| {
                    ui_json_fields::write(*morph, out)
                        .map(ArtifactFields::Json)
                        .map_err(GfxError::Write)
                },
            )),
            Job::MissingBase(order, sub) => {
                JobResult::single(Outcome::bitmap(*order, sub.id, Err(self.cut_error(sub))))
            }
            Job::Bitmaps { base, cuts } => {
                self.write_bitmaps(assets, *base, cuts, output, budget, files)
            }
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
