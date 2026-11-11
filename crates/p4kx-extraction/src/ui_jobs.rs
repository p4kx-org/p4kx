// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

use p4kx_gfx::{Assets, Bitmap, BitmapSource, GfxError, MorphShape, RgbaImage, Shape};
use p4kx_texture::{MipByteSpans, RgbaImage as DdsImage};

use crate::budget::{Budget, Lease};
use crate::output_file::write_error;
use crate::texture_preview_png::input::PreviewInputs;
use crate::ui_external::{self, ExternalPath};
use crate::{
    ConversionError, DirectoryCommit, DirectoryReceipt, DiskTextures, FieldPath, Texture,
    UiAssetKind, UiMovie, WrittenUiAsset,
};

/// A definition, or a bitmap with the sub-images that cut it, written by one parallel job.
pub(crate) enum Job<'b, 'c> {
    /// A bitmap and the sub-images that cut it, each with its place in file order.
    Bitmaps {
        base: (usize, &'b Bitmap<'c>),
        cuts: Vec<(usize, &'b Bitmap<'c>)>,
    },
    /// A sub-image whose bitmap the movie does not define.
    MissingBase(usize, &'b Bitmap<'c>),
    /// A shape.
    Shape(usize, &'b Shape),
    /// A morph shape.
    MorphShape(usize, &'b MorphShape),
}

pub(crate) struct UiJobOutput<'owner, 'source> {
    pub(crate) folder: &'owner Path,
    pub(crate) publication: &'owner DirectoryReceipt<'source>,
}

pub(crate) struct Outcome<'owner> {
    pub(crate) order: usize,
    pub(crate) kind: UiAssetKind,
    pub(crate) id: u16,
    pub(crate) result: Result<CommittedUiAsset<'owner>, ConversionError>,
}

pub(crate) struct CommittedUiAsset<'owner> {
    pub(crate) written: WrittenUiAsset,
    pub(crate) fields: ArtifactFields,
    pub(crate) receipt: DirectoryCommit<'owner>,
}

pub(crate) enum ArtifactFields {
    Png,
    Json(Vec<FieldPath>),
}

pub(crate) struct JobResult<'owner, 'archive> {
    pub(crate) outcomes: Vec<Outcome<'owner>>,
    pub(crate) bitmap: Option<BitmapRead<'archive>>,
}

pub(crate) struct BitmapRead<'archive> {
    pub(crate) order: usize,
    pub(crate) pixels: BitmapPixels<'archive>,
}

pub(crate) enum BitmapPixels<'archive> {
    Embedded {
        used_jpeg_tables: bool,
    },
    External {
        selection: ExternalPath,
        inputs: Box<PreviewInputs<'archive, MipByteSpans>>,
    },
}

struct DecodedBitmap<'budget, 'archive> {
    image: RgbaImage,
    _lease: Lease<'budget>,
    pixels: BitmapPixels<'archive>,
}

impl<'owner> Outcome<'owner> {
    pub(crate) const fn bitmap(
        order: usize,
        id: u16,
        result: Result<CommittedUiAsset<'owner>, ConversionError>,
    ) -> Self {
        Self {
            order,
            kind: UiAssetKind::Bitmap,
            id,
            result,
        }
    }
}

impl<'owner> JobResult<'owner, '_> {
    pub(crate) fn single(outcome: Outcome<'owner>) -> Self {
        Self {
            outcomes: vec![outcome],
            bitmap: None,
        }
    }
}

/// Returns the jobs of a movie's definitions, the first of each kind and ID only, with every
/// sub-image grouped under the bitmap it cuts.
pub(crate) fn jobs<'b, 'c>(assets: &'b Assets<'c>) -> Vec<Job<'b, 'c>> {
    let mut jobs = Vec::new();
    let mut bases: HashMap<u16, usize> = HashMap::new();
    let mut cuts = Vec::new();
    let mut seen = HashSet::new();
    for (order, bitmap) in assets.bitmaps.iter().enumerate() {
        if !seen.insert(bitmap.id) {
            continue;
        }
        if let BitmapSource::SubImage(sub) = &bitmap.source {
            cuts.push((order, sub.image_id, bitmap));
        } else {
            bases.insert(bitmap.id, jobs.len());
            jobs.push(Job::Bitmaps {
                base: (order, bitmap),
                cuts: Vec::new(),
            });
        }
    }
    for (order, image_id, bitmap) in cuts {
        match bases.get(&image_id).and_then(|&index| jobs.get_mut(index)) {
            Some(Job::Bitmaps { cuts, .. }) => cuts.push((order, bitmap)),
            _ => jobs.push(Job::MissingBase(order, bitmap)),
        }
    }
    let mut order = assets.bitmaps.len();
    let mut seen = HashSet::new();
    for shape in &assets.shapes {
        if seen.insert(shape.id) {
            jobs.push(Job::Shape(order, shape));
        }
        order += 1;
    }
    seen.clear();
    for morph in &assets.morph_shapes {
        if seen.insert(morph.id) {
            jobs.push(Job::MorphShape(order, morph));
        }
        order += 1;
    }
    jobs
}

impl<'archive> UiMovie<'archive> {
    pub(crate) fn write_file<'owner>(
        &self,
        order: usize,
        output: &UiJobOutput<'owner, '_>,
        kind: UiAssetKind,
        id: u16,
        write: impl FnOnce(&mut dyn Write) -> Result<ArtifactFields, GfxError>,
    ) -> Outcome<'owner> {
        let basename = format!("{}_{id}.{}", kind.name(), kind.extension());
        let path = output.folder.join(&basename);
        let result = output
            .publication
            .write(OsStr::new(&basename), |writer| {
                write(writer).map_err(|source| match source {
                    GfxError::Write(source) => write_error(&path, source),
                    source => self.asset_error(id, source),
                })
            })
            .map(|(fields, receipt)| CommittedUiAsset {
                written: WrittenUiAsset {
                    kind,
                    id,
                    size: receipt.byte_length(),
                    path,
                },
                fields,
                receipt,
            });
        Outcome {
            order,
            kind,
            id,
            result,
        }
    }

    pub(crate) fn write_bitmaps<'owner>(
        &self,
        assets: &Assets<'_>,
        (order, base): (usize, &Bitmap<'_>),
        cuts: &[(usize, &Bitmap<'_>)],
        output: &UiJobOutput<'owner, '_>,
        budget: &Budget,
        files: &DiskTextures,
    ) -> JobResult<'owner, 'archive> {
        let png = |order, id, image: &RgbaImage| {
            self.write_file(order, output, UiAssetKind::Bitmap, id, |out| {
                image.write_png(out).map(|()| ArtifactFields::Png)
            })
        };
        let decoded =
            match self.decode_bitmap(assets, base, budget, files) {
                Ok(decoded) => decoded,
                Err(error) => {
                    let mut outcomes = vec![Outcome::bitmap(order, base.id, Err(error))];
                    outcomes.extend(cuts.iter().map(|&(order, cut)| {
                        Outcome::bitmap(order, cut.id, Err(self.cut_error(cut)))
                    }));
                    return JobResult {
                        outcomes,
                        bitmap: None,
                    };
                }
            };
        let mut outcomes = vec![png(order, base.id, &decoded.image)];
        for &(order, cut) in cuts {
            let BitmapSource::SubImage(sub) = &cut.source else {
                continue;
            };
            outcomes.push(
                decoded
                    .image
                    .crop(
                        (u32::from(sub.x1), u32::from(sub.y1)),
                        (u32::from(sub.x2), u32::from(sub.y2)),
                    )
                    .map_or_else(
                        || Outcome::bitmap(order, cut.id, Err(self.cut_error(cut))),
                        |piece| png(order, cut.id, &piece),
                    ),
            );
        }
        JobResult {
            outcomes,
            bitmap: Some(BitmapRead {
                order,
                pixels: decoded.pixels,
            }),
        }
    }

    /// Decodes the first mip of the texture of the external image `id`.
    pub(crate) fn texture_image(
        &self,
        id: u16,
        texture: &Texture<'_>,
    ) -> Result<RgbaImage, ConversionError> {
        self.rgba_image(id, texture.decode_mip(0)?)
    }

    fn rgba_image(&self, id: u16, decoded: DdsImage) -> Result<RgbaImage, ConversionError> {
        let (width, height) = (decoded.width(), decoded.height());
        RgbaImage::from_pixels(width, height, decoded.into_pixels())
            .map_err(|source| self.asset_error(id, source))
    }

    fn decode_bitmap<'budget>(
        &self,
        assets: &Assets<'_>,
        bitmap: &Bitmap<'_>,
        budget: &'budget Budget,
        files: &DiskTextures,
    ) -> Result<DecodedBitmap<'budget, 'archive>, ConversionError> {
        let gfx = |source| self.asset_error(bitmap.id, source);
        match &bitmap.source {
            BitmapSource::External(image) => {
                let texture = ui_external::open_external(self, bitmap.id, image, files)?;
                let header = texture.header();
                let lease = budget.take(u64::from(header.width()) * u64::from(header.height()));
                let (decoded, inputs) = texture.decode_mip_with_inputs(0)?;
                let selection = ExternalPath::select(image);
                Ok(DecodedBitmap {
                    image: self.rgba_image(bitmap.id, decoded)?,
                    _lease: lease,
                    pixels: BitmapPixels::External {
                        selection,
                        inputs: Box::new(inputs),
                    },
                })
            }
            BitmapSource::SubImage(_) => Err(self.cut_error(bitmap)),
            BitmapSource::Lossless(_) | BitmapSource::Jpeg(_) => {
                let (width, height) = assets.bitmap_size(bitmap).map_err(gfx)?;
                let lease = budget.take(u64::from(width) * u64::from(height));
                let decoded = assets.decode_bitmap_observed(bitmap).map_err(gfx)?;
                Ok(DecodedBitmap {
                    image: decoded.image,
                    _lease: lease,
                    pixels: BitmapPixels::Embedded {
                        used_jpeg_tables: decoded.used_jpeg_tables,
                    },
                })
            }
        }
    }
}
