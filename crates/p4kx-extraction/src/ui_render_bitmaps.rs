// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_gfx::{Assets, Bitmap, BitmapSource, RgbaImage, SubImage};
use rayon::prelude::*;

use crate::budget::Budget;
use crate::ui_external::{self, ExternalPath};
use crate::ui_service::ui_bitmap_decode::{BitmapRead, DecodedPixels};
use crate::{
    ConversionError, DiskTextures, MAX_UI_RENDER_BITMAP_PIXELS, Texture, UiBitmapFailure, UiMovie,
};

/// A bitmap the frame needs and the bitmaps of the frame cut from it, in ascending ID order.
struct Base<'b, 'c> {
    bitmap: &'b Bitmap<'c>,
    order: usize,
    users: Vec<User>,
}

#[derive(Clone, Copy)]
struct User {
    id: u16,
    order: usize,
    cut: Option<SubImage>,
}

/// Where the pixels of a base come from, opened but not decoded.
enum Source<'a, 'b, 'c> {
    Embedded(&'b Bitmap<'c>),
    External {
        texture: Box<Texture<'a>>,
        selection: ExternalPath,
    },
}

/// A base whose pixels may be kept, with its source and pixel count.
type Granted<'a, 'b, 'c> = (Base<'b, 'c>, Source<'a, 'b, 'c>, u64);

#[derive(Default)]
struct Decoded<'archive> {
    pictures: Vec<(User, RgbaImage)>,
    failures: Vec<UiBitmapFailure>,
    source: Option<BitmapRead<'archive>>,
}

struct BitmapLocation {
    order: usize,
    group: usize,
}

/// The bitmaps a rendered frame fills with and their observed definitions.
pub(crate) struct FrameBitmaps<'archive> {
    pub(crate) images: HashMap<u16, RgbaImage>,
    pub(crate) failures: Vec<UiBitmapFailure>,
    sources: Vec<BitmapRead<'archive>>,
    locations: HashMap<u16, BitmapLocation>,
}

impl<'archive> FrameBitmaps<'archive> {
    pub(crate) fn source(&self, bitmap: u16) -> Option<(usize, &BitmapRead<'archive>)> {
        let location = self.locations.get(&bitmap)?;
        self.sources
            .get(location.group)
            .map(|base| (location.order, base))
    }
}

impl<'a> UiMovie<'a> {
    /// Decodes the bitmaps `ids` in parallel, each bitmap that sub-images cut once; the kept
    /// images hold at most [`MAX_UI_RENDER_BITMAP_PIXELS`], granted in ascending ID order.
    pub(crate) fn frame_bitmaps(&self, assets: &Assets<'_>, ids: &[u16]) -> FrameBitmaps<'a> {
        let mut failures = Vec::new();
        let bases = self.group(assets, ids, &mut failures);
        let files = DiskTextures::default();
        let granted = self.grant(assets, bases, &mut failures, &files);
        let budget = Budget::new(MAX_UI_RENDER_BITMAP_PIXELS);
        let decoded: Vec<Decoded<'a>> = granted
            .into_par_iter()
            .map(|(base, source, pixels)| self.decode(assets, &base, source, pixels, &budget))
            .collect();
        let mut result = FrameBitmaps {
            images: HashMap::with_capacity(ids.len()),
            failures,
            sources: Vec::new(),
            locations: HashMap::with_capacity(ids.len()),
        };
        for decoded in decoded {
            result.failures.extend(decoded.failures);
            if let Some(source) = decoded.source {
                let group = result.sources.len();
                result.sources.push(source);
                for (user, image) in decoded.pictures {
                    result.images.insert(user.id, image);
                    result.locations.insert(
                        user.id,
                        BitmapLocation {
                            order: user.order,
                            group,
                        },
                    );
                }
            }
        }
        result.failures.sort_by_key(|failure| failure.id);
        result
    }

    /// Groups the bitmaps `ids` under the bitmap each is or is cut from.
    fn group<'b, 'c>(
        &self,
        assets: &'b Assets<'c>,
        ids: &[u16],
        failures: &mut Vec<UiBitmapFailure>,
    ) -> BTreeMap<u16, Base<'b, 'c>> {
        let mut by_id = HashMap::with_capacity(assets.bitmaps.len());
        for (order, bitmap) in assets.bitmaps.iter().enumerate() {
            by_id.entry(bitmap.id).or_insert((order, bitmap));
        }
        let mut bases: BTreeMap<u16, Base<'_, '_>> = BTreeMap::new();
        for &id in ids {
            let Some(&(order, bitmap)) = by_id.get(&id) else {
                let movie = self.name().to_owned();
                let error = ConversionError::UiBitmapMissing { movie, id };
                failures.push(UiBitmapFailure { id, error });
                continue;
            };
            let (base_order, base, cut) = match &bitmap.source {
                BitmapSource::SubImage(sub) => match by_id.get(&sub.image_id) {
                    Some(&(base_order, base))
                        if !matches!(base.source, BitmapSource::SubImage(_)) =>
                    {
                        (base_order, base, Some(*sub))
                    }
                    _ => {
                        let error = self.cut_error(bitmap);
                        failures.push(UiBitmapFailure { id, error });
                        continue;
                    }
                },
                _ => (order, bitmap, None),
            };
            bases
                .entry(base.id)
                .or_insert_with(|| Base {
                    bitmap: base,
                    order: base_order,
                    users: Vec::new(),
                })
                .users
                .push(User { id, order, cut });
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
                        for user in &base.users {
                            let id = user.id;
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
        for user in &base.users {
            let id = user.id;
            let error = if user.cut.is_none()
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
            return Ok((
                Source::External {
                    texture: Box::new(texture),
                    selection: ExternalPath::select(image),
                },
                pixels,
            ));
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
        source: Source<'a, '_, '_>,
        pixels: u64,
        budget: &Budget,
    ) -> Decoded<'a> {
        let id = base.bitmap.id;
        let decoded = {
            let _lease = budget.take(pixels);
            match source {
                Source::Embedded(bitmap) => self.embedded_bitmap_pixels(assets, bitmap),
                Source::External { texture, selection } => {
                    self.external_bitmap_pixels(id, &texture, selection)
                }
            }
        };
        match decoded {
            Ok(decoded) => self.produce_frame_bitmaps(base, decoded),
            Err(error) => {
                let mut result = Decoded::default();
                self.fail_users(base, error, &mut result.failures);
                result
            }
        }
    }

    fn produce_frame_bitmaps(
        &self,
        base: &Base<'_, '_>,
        decoded: DecodedPixels<'a>,
    ) -> Decoded<'a> {
        let mut whole = None;
        let mut result = Decoded {
            pictures: Vec::with_capacity(base.users.len()),
            failures: Vec::new(),
            source: Some(BitmapRead {
                order: base.order,
                pixels: decoded.pixels,
            }),
        };
        for &user in &base.users {
            let Some(sub) = user.cut else {
                whole = Some(user);
                continue;
            };
            if let Some(piece) = decoded.image.crop(
                (u32::from(sub.x1), u32::from(sub.y1)),
                (u32::from(sub.x2), u32::from(sub.y2)),
            ) {
                result.pictures.push((user, piece));
            } else {
                result.failures.push(UiBitmapFailure {
                    id: user.id,
                    error: ConversionError::UiSubImage {
                        movie: self.name().to_owned(),
                        id: user.id,
                        image_id: base.bitmap.id,
                    },
                });
            }
        }
        if let Some(user) = whole {
            result.pictures.push((user, decoded.image));
        }
        result
    }
}

/// Returns the pixels a base keeps: its own when the frame fills with it, and its cuts.
fn kept_pixels(base: &Base<'_, '_>, pixels: u64) -> u64 {
    base.users
        .iter()
        .map(|user| {
            user.cut.map_or(pixels, |sub| {
                u64::from(sub.x2.saturating_sub(sub.x1)) * u64::from(sub.y2.saturating_sub(sub.y1))
            })
        })
        .fold(0_u64, u64::saturating_add)
}
