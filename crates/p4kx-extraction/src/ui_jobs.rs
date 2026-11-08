// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use p4kx_gfx::{Assets, Bitmap, BitmapSource, GfxError, MorphShape, RgbaImage, Shape};

use crate::budget::{Budget, Lease};
use crate::output_file::{write_error, write_replacing_protected};
use crate::ui_external;
use crate::{ConversionError, DiskTextures, Texture, UiAssetKind, UiMovie, WrittenUiAsset};

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

/// What one definition's job did, with its place in file order.
pub(crate) struct Outcome {
    pub(crate) order: usize,
    pub(crate) kind: UiAssetKind,
    pub(crate) id: u16,
    pub(crate) result: Result<WrittenUiAsset, ConversionError>,
}

impl Outcome {
    pub(crate) const fn bitmap(
        order: usize,
        id: u16,
        result: Result<WrittenUiAsset, ConversionError>,
    ) -> Self {
        Self {
            order,
            kind: UiAssetKind::Bitmap,
            id,
            result,
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

impl UiMovie<'_> {
    /// Writes one definition's file through `write`, replacing it.
    pub(crate) fn write_file(
        &self,
        order: usize,
        folder: &Path,
        kind: UiAssetKind,
        id: u16,
        write: impl FnOnce(&mut BufWriter<File>) -> Result<(), GfxError>,
    ) -> Outcome {
        let path = folder.join(format!("{}_{id}.{}", kind.name(), kind.extension()));
        let result = self
            .write_protection()
            .and_then(|protection| {
                write_replacing_protected(&path, protection, None, |writer| {
                    write(writer).map_err(|source| match source {
                        GfxError::Write(source) => write_error(&path, source),
                        source => self.asset_error(id, source),
                    })
                })
            })
            .and_then(|()| fs::metadata(&path).map_err(|source| write_error(&path, source)))
            .map(|metadata| WrittenUiAsset {
                kind,
                id,
                size: metadata.len(),
                path: path.clone(),
            });
        Outcome {
            order,
            kind,
            id,
            result,
        }
    }

    /// Decodes a bitmap once within the pixel budget and writes it and the sub-images cutting it.
    pub(crate) fn write_bitmaps(
        &self,
        assets: &Assets<'_>,
        (order, base): (usize, &Bitmap<'_>),
        cuts: &[(usize, &Bitmap<'_>)],
        folder: &Path,
        budget: &Budget,
        files: &DiskTextures,
    ) -> Vec<Outcome> {
        let png = |order, id, image: &RgbaImage| {
            self.write_file(order, folder, UiAssetKind::Bitmap, id, |out| {
                image.write_png(out)
            })
        };
        let (image, _lease) =
            match self.decode_bitmap(assets, base, budget, files) {
                Ok(decoded) => decoded,
                Err(error) => {
                    let mut outcomes = vec![Outcome::bitmap(order, base.id, Err(error))];
                    outcomes.extend(cuts.iter().map(|&(order, cut)| {
                        Outcome::bitmap(order, cut.id, Err(self.cut_error(cut)))
                    }));
                    return outcomes;
                }
            };
        let mut outcomes = vec![png(order, base.id, &image)];
        for &(order, cut) in cuts {
            let BitmapSource::SubImage(sub) = &cut.source else {
                continue;
            };
            outcomes.push(
                image
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
        outcomes
    }

    /// Decodes the first mip of the texture of the external image `id`.
    pub(crate) fn texture_image(
        &self,
        id: u16,
        texture: &Texture<'_>,
    ) -> Result<RgbaImage, ConversionError> {
        let decoded = texture.decode_mip(0)?;
        let (width, height) = (decoded.width(), decoded.height());
        RgbaImage::from_pixels(width, height, decoded.into_pixels())
            .map_err(|source| self.asset_error(id, source))
    }

    fn decode_bitmap<'b>(
        &self,
        assets: &Assets<'_>,
        bitmap: &Bitmap<'_>,
        budget: &'b Budget,
        files: &DiskTextures,
    ) -> Result<(RgbaImage, Lease<'b>), ConversionError> {
        let gfx = |source| self.asset_error(bitmap.id, source);
        match &bitmap.source {
            BitmapSource::External(image) => {
                let texture = ui_external::open_external(self, bitmap.id, image, files)?;
                let header = texture.header();
                let lease = budget.take(u64::from(header.width()) * u64::from(header.height()));
                Ok((self.texture_image(bitmap.id, &texture)?, lease))
            }
            BitmapSource::SubImage(_) => Err(self.cut_error(bitmap)),
            BitmapSource::Lossless(_) | BitmapSource::Jpeg(_) => {
                let (width, height) = assets.bitmap_size(bitmap).map_err(gfx)?;
                let lease = budget.take(u64::from(width) * u64::from(height));
                Ok((assets.decode_bitmap(bitmap).map_err(gfx)?, lease))
            }
        }
    }
}
