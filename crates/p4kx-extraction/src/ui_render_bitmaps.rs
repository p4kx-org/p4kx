// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_gfx::{Assets, Bitmap, BitmapSource, RgbaImage, SubImage};
use rayon::prelude::*;

use crate::budget::Budget;
use crate::ui_external;
use crate::{
    ConversionError, DiskTextures, MAX_UI_RENDER_BITMAP_PIXELS, Texture, UiBitmapFailure, UiMovie,
};

/// A bitmap the frame needs and the bitmaps of the frame cut from it, in ascending ID order.
struct Base<'b, 'c> {
    bitmap: &'b Bitmap<'c>,
    users: Vec<(u16, Option<SubImage>)>,
}

/// Where the pixels of a base come from, opened but not decoded.
enum Source<'a, 'b, 'c> {
    Embedded(&'b Bitmap<'c>),
    External(Box<Texture<'a>>),
}

/// A base whose pixels may be kept, with its source and pixel count.
type Granted<'a, 'b, 'c> = (Base<'b, 'c>, Source<'a, 'b, 'c>, u64);

/// The decoded images of the users of one base, or why each failed.
type Decoded = Vec<(u16, Result<RgbaImage, ConversionError>)>;

/// The bitmaps a rendered frame fills with, decoded, and those that could not be.
pub(crate) struct FrameBitmaps {
    pub(crate) images: HashMap<u16, RgbaImage>,
    pub(crate) failures: Vec<UiBitmapFailure>,
}

impl<'a> UiMovie<'a> {
    /// Decodes the bitmaps `ids` in parallel, each bitmap that sub-images cut once; the kept
    /// images hold at most [`MAX_UI_RENDER_BITMAP_PIXELS`], granted in ascending ID order.
    pub(crate) fn frame_bitmaps(&self, assets: &Assets<'_>, ids: &[u16]) -> FrameBitmaps {
        let mut failures = Vec::new();
        let bases = self.group(assets, ids, &mut failures);
        let files = DiskTextures::default();
        let granted = self.grant(assets, bases, &mut failures, &files);
        let budget = Budget::new(MAX_UI_RENDER_BITMAP_PIXELS);
        let decoded: Vec<Decoded> = granted
            .into_par_iter()
            .map(|(base, source, pixels)| self.decode(assets, &base, source, pixels, &budget))
            .collect();
        let mut images = HashMap::with_capacity(ids.len());
        for (id, result) in decoded.into_iter().flatten() {
            match result {
                Ok(image) => {
                    images.insert(id, image);
                }
                Err(error) => failures.push(UiBitmapFailure { id, error }),
            }
        }
        failures.sort_by_key(|failure| failure.id);
        FrameBitmaps { images, failures }
    }

    /// Groups the bitmaps `ids` under the bitmap each is or is cut from.
    fn group<'b, 'c>(
        &self,
        assets: &'b Assets<'c>,
        ids: &[u16],
        failures: &mut Vec<UiBitmapFailure>,
    ) -> BTreeMap<u16, Base<'b, 'c>> {
        let mut by_id: HashMap<u16, &Bitmap<'_>> = HashMap::with_capacity(assets.bitmaps.len());
        for bitmap in &assets.bitmaps {
            by_id.entry(bitmap.id).or_insert(bitmap);
        }
        let mut bases: BTreeMap<u16, Base<'_, '_>> = BTreeMap::new();
        for &id in ids {
            let Some(&bitmap) = by_id.get(&id) else {
                let movie = self.name().to_owned();
                let error = ConversionError::UiBitmapMissing { movie, id };
                failures.push(UiBitmapFailure { id, error });
                continue;
            };
            let (base, cut) = match &bitmap.source {
                BitmapSource::SubImage(sub) => match by_id.get(&sub.image_id) {
                    Some(&base) if !matches!(base.source, BitmapSource::SubImage(_)) => {
                        (base, Some(*sub))
                    }
                    _ => {
                        let error = self.cut_error(bitmap);
                        failures.push(UiBitmapFailure { id, error });
                        continue;
                    }
                },
                _ => (bitmap, None),
            };
            bases
                .entry(base.id)
                .or_insert_with(|| Base {
                    bitmap: base,
                    users: Vec::new(),
                })
                .users
                .push((id, cut));
        }
        bases
    }

    /// Opens the bases in parallel and grants their pixels in ascending ID order.
    fn grant<'b, 'c>(
        &self,
        assets: &Assets<'_>,
        bases: BTreeMap<u16, Base<'b, 'c>>,
        failures: &mut Vec<UiBitmapFailure>,
        files: &DiskTextures,
    ) -> Vec<Granted<'a, 'b, 'c>> {
        let opened: Vec<(Base<'b, 'c>, Result<_, ConversionError>)> = bases
            .into_values()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|base| {
                let source = self.open(assets, base.bitmap, files);
                (base, source)
            })
            .collect();
        let mut granted = Vec::with_capacity(opened.len());
        let mut kept = 0_u64;
        for (base, opened) in opened {
            match opened {
                Ok((source, pixels)) => {
                    let needed = kept_pixels(&base, pixels);
                    if kept.saturating_add(needed) <= MAX_UI_RENDER_BITMAP_PIXELS {
                        kept += needed;
                        granted.push((base, source, pixels));
                    } else {
                        for &(id, _) in &base.users {
                            let error = ConversionError::UiRenderBitmaps {
                                movie: self.name().to_owned(),
                                id,
                                limit: MAX_UI_RENDER_BITMAP_PIXELS,
                            };
                            failures.push(UiBitmapFailure { id, error });
                        }
                    }
                }
                Err(error) => self.fail_users(&base, error, failures),
            }
        }
        granted
    }

    /// Lists every user of a base as failed: the base itself with `error`, its cuts as cuts of a
    /// bitmap that cannot be decoded.
    fn fail_users(
        &self,
        base: &Base<'_, '_>,
        error: ConversionError,
        failures: &mut Vec<UiBitmapFailure>,
    ) {
        let mut error = Some(error);
        for &(id, cut) in &base.users {
            let error = if cut.is_none()
                && let Some(error) = error.take()
            {
                error
            } else {
                ConversionError::UiSubImage {
                    movie: self.name().to_owned(),
                    id,
                    image_id: base.bitmap.id,
                }
            };
            failures.push(UiBitmapFailure { id, error });
        }
    }

    fn open<'b, 'c>(
        &self,
        assets: &Assets<'_>,
        bitmap: &'b Bitmap<'c>,
        files: &DiskTextures,
    ) -> Result<(Source<'a, 'b, 'c>, u64), ConversionError> {
        if let BitmapSource::External(image) = &bitmap.source {
            let texture = ui_external::open_external(self, bitmap.id, image, files)?;
            let header = texture.header();
            let pixels = u64::from(header.width()) * u64::from(header.height());
            return Ok((Source::External(Box::new(texture)), pixels));
        }
        let (width, height) = assets
            .bitmap_size(bitmap)
            .map_err(|source| self.asset_error(bitmap.id, source))?;
        Ok((
            Source::Embedded(bitmap),
            u64::from(width) * u64::from(height),
        ))
    }

    fn decode(
        &self,
        assets: &Assets<'_>,
        base: &Base<'_, '_>,
        source: Source<'_, '_, '_>,
        pixels: u64,
        budget: &Budget,
    ) -> Decoded {
        let id = base.bitmap.id;
        let image = {
            let _lease = budget.take(pixels);
            match source {
                Source::Embedded(bitmap) => assets
                    .decode_bitmap(bitmap)
                    .map_err(|source| self.asset_error(id, source)),
                Source::External(texture) => self.texture_image(id, &texture),
            }
        };
        let image = match image {
            Ok(image) => image,
            Err(error) => {
                let mut failures = Vec::new();
                self.fail_users(base, error, &mut failures);
                return failures
                    .into_iter()
                    .map(|failure| (failure.id, Err(failure.error)))
                    .collect();
            }
        };
        let mut whole = None;
        let mut results = Vec::with_capacity(base.users.len());
        for &(user, cut) in &base.users {
            let Some(sub) = cut else {
                whole = Some(user);
                continue;
            };
            let piece = image
                .crop(
                    (u32::from(sub.x1), u32::from(sub.y1)),
                    (u32::from(sub.x2), u32::from(sub.y2)),
                )
                .ok_or_else(|| ConversionError::UiSubImage {
                    movie: self.name().to_owned(),
                    id: user,
                    image_id: id,
                });
            results.push((user, piece));
        }
        if let Some(user) = whole {
            results.push((user, Ok(image)));
        }
        results
    }
}

/// Returns the pixels a base keeps: its own when the frame fills with it, and its cuts.
fn kept_pixels(base: &Base<'_, '_>, pixels: u64) -> u64 {
    base.users
        .iter()
        .map(|&(_, cut)| {
            cut.map_or(pixels, |sub| {
                u64::from(sub.x2.saturating_sub(sub.x1)) * u64::from(sub.y2.saturating_sub(sub.y1))
            })
        })
        .fold(0_u64, u64::saturating_add)
}
