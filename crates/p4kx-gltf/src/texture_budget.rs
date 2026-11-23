// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::texture_cache::{MAX_DECODE_PIXELS, SourceStore, paths};
use crate::texture_plan::{Failure, Job, Key, MAX_TEXTURE_IMAGES, MAX_TEXTURE_PIXELS};
use crate::texture_report::TextureRole;
use crate::texture_roles::Source;

/// How many images and pixels the images of one export may hold.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Budget {
    pub(crate) images: usize,
    pub(crate) pixels: u64,
}

impl Budget {
    pub(crate) const DEFAULT: Self = Self {
        images: MAX_TEXTURE_IMAGES,
        pixels: MAX_TEXTURE_PIXELS,
    };
}

/// What the budget says about a job before it runs.
#[derive(Debug, Clone)]
pub(crate) enum Verdict {
    /// The image is known not to fit the pixels left: the job does not run.
    Refused,
    /// The export holds as many images as allowed: the job does not run.
    Spent,
    /// The textures of a layered image decode more than the decode limit together: the job does
    /// not run.
    TooLarge,
    /// The texture that sizes the image cannot be loaded: the job does not run.
    Failed(Failure),
    /// The image is known to fit with this many pixels, which the tally holds back for it.
    Reserved(u64),
    /// The size is not known: the image is checked once it is made.
    Unknown,
    /// The image is not needed, as the one it stands in for was made: the job does not run and
    /// takes no image slot.
    Skipped,
}

impl Verdict {
    /// Returns whether the job runs.
    pub(crate) const fn runs(&self) -> bool {
        matches!(self, Self::Reserved(_) | Self::Unknown)
    }
}

/// The images and pixels the images made so far hold.
#[derive(Debug)]
pub(crate) struct Tally {
    budget: Budget,
    images: usize,
    pixels: u64,
}

impl Tally {
    pub(crate) const fn new(budget: Budget) -> Self {
        Self {
            budget,
            images: 0,
            pixels: 0,
        }
    }

    /// Returns whether the export holds as many images as it may.
    pub(crate) const fn images_spent(&self) -> bool {
        self.images >= self.budget.images
    }

    const fn spent(&self) -> bool {
        self.images_spent() || self.pixels >= self.budget.pixels
    }

    /// Returns whether one more image of `size` pixels fits.
    pub(crate) const fn fits(&self, size: u64) -> bool {
        self.images < self.budget.images && self.pixels.saturating_add(size) <= self.budget.pixels
    }

    /// Counts an image of `size` pixels.
    pub(crate) const fn take(&mut self, size: u64) {
        self.images += 1;
        self.pixels = self.pixels.saturating_add(size);
    }

    /// Uncounts an image of `size` pixels that `take` counted.
    pub(crate) const fn give_back(&mut self, size: u64) {
        self.images = self.images.saturating_sub(1);
        self.pixels = self.pixels.saturating_sub(size);
    }

    fn reserve(&mut self, (width, height): (u32, u32), if_too_large: Verdict) -> Verdict {
        let size = u64::from(width) * u64::from(height);
        if self.fits(size) {
            self.take(size);
            Verdict::Reserved(size)
        } else {
            if_too_large
        }
    }

    /// Decides from the header sizes whether `job` may run, and holds back its pixels if so.
    pub(crate) fn plan(&mut self, job: &Job, store: &mut SourceStore<'_>) -> Verdict {
        if self.images_spent() {
            return Verdict::Spent;
        }
        if self.spent() {
            return Verdict::Refused;
        }
        if job.layered.is_some() {
            return self.plan_layered(job, store);
        }
        if job.normal.is_none()
            && !job.explicit_gloss.is_empty()
            && let Some(specular) = &job.specular
        {
            // A failed specular decode can leave a smaller gloss-only image.
            return match store.dimensions(&specular.path) {
                Ok(Some(size)) => self.reserve(size, Verdict::Unknown),
                Ok(None) | Err(_) => Verdict::Unknown,
            };
        }
        let Some((role, source)) = sized_by(job) else {
            return Verdict::Unknown;
        };
        match store.dimensions(&source.path) {
            Ok(Some(size)) => self.reserve(size, Verdict::Refused),
            Ok(None) if matches!(job.key, Key::FactoryBowSpecularColour { .. }) => {
                Verdict::Failed(Failure {
                    role,
                    file: source.file.clone(),
                    reason: "the optional specular colour has no verified dimensions".to_owned(),
                })
            }
            Ok(None) => Verdict::Unknown,
            Err(error) => Verdict::Failed(Failure {
                role,
                file: source.file.clone(),
                reason: error.to_string(),
            }),
        }
    }

    /// Decides for a layered job: its textures must decode within the limit together, and its
    /// image has the size of the first of its maps, the blend map first, whose header can be read;
    /// a map that cannot be read is left to the job to report.
    fn plan_layered(&mut self, job: &Job, store: &mut SourceStore<'_>) -> Verdict {
        if store.header_pixels(paths(job)) > MAX_DECODE_PIXELS {
            return Verdict::TooLarge;
        }
        let Some(layered) = &job.layered else {
            return Verdict::Unknown;
        };
        let maps = job.layer_maps();
        let mut failure = None;
        for (role, source) in layered.size_sources(maps) {
            match store.dimensions(&source.path) {
                Ok(Some(size)) => return self.reserve(size, Verdict::Refused),
                Ok(None) => return Verdict::Unknown,
                Err(error) => {
                    failure.get_or_insert_with(|| Failure {
                        role,
                        file: if role == TextureRole::Specular {
                            source.file.clone()
                        } else {
                            job.file_of(source)
                        },
                        reason: error.to_string(),
                    });
                }
            }
        }
        failure.map_or(Verdict::Unknown, Verdict::Failed)
    }
}

/// The source whose size the job's image has, when that is known before anything is loaded.
fn sized_by(job: &Job) -> Option<(TextureRole, &Source)> {
    let diffuse = job.diffuse.as_ref().map(|s| (TextureRole::Diffuse, s));
    let normal = job.normal.as_ref().map(|s| (TextureRole::Normal, s));
    let specular = job.specular.as_ref().map(|s| (TextureRole::Specular, s));
    match &job.key {
        Key::LayerBase { .. } | Key::LayerNormal { .. } | Key::LayerFinish { .. } => None,
        Key::Base { .. } => diffuse,
        Key::Normal { .. } => normal,
        Key::FactoryBowSpecularColour { .. } => specular,
        Key::MetallicRoughness { .. } => {
            let mut candidates = normal.into_iter().chain(specular).chain(
                job.explicit_gloss
                    .iter()
                    .map(|source| (TextureRole::Gloss, source)),
            );
            let first = candidates.next()?;
            candidates.next().is_none().then_some(first)
        }
    }
}
