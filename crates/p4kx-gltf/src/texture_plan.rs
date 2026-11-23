// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_texture::DdsTransfer;

use crate::layer_jobs::{fallback_guards, layer_normals_last};
use crate::layer_plan::{LayerMaps, Layered};
use crate::materials::{MaterialPlan, Slot, display_name};
use crate::surface_finish::SurfaceFinish;
use crate::texture_assemble::Wish;
use crate::texture_loader::{TextureLoader, WithFallbacks};
use crate::texture_make::{Budget, run_guarded};
use crate::texture_origins::{TextureMaterialSource, TextureReadUse};
use crate::texture_pixels::Tints;
use crate::texture_report::{TextureFallback, TextureRole};
use crate::texture_roles::Source;

mod jobs;

/// The most images one GLB embeds.
pub const MAX_TEXTURE_IMAGES: usize = 256;

/// The most pixels all the images of one GLB hold together.
pub const MAX_TEXTURE_PIXELS: u64 = 1 << 29;

/// What an image of the GLB is made of; equal keys give equal images, which are embedded once.
///
/// A specular map brings the material's colours into the image, because a metal's colour is its
/// tinted specular colour; the key then holds them, quantised to 8 bits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Base {
        diffuse: String,
        specular: Option<(String, [u8; 3])>,
        tint: [u8; 3],
        decal: Option<[[u8; 3]; 3]>,
        keep_alpha: bool,
        opacity_source: bool,
    },
    FactoryBowSpecularColour {
        source: String,
        level_scale: u32,
    },
    Normal {
        normal: String,
    },
    MetallicRoughness {
        finish: Option<SurfaceFinish>,
        gloss: Option<String>,
        explicit_gloss: Vec<String>,
        specular: Option<(String, [u8; 3])>,
    },
    LayerBase {
        blend: String,
    },
    LayerFinish {
        blend: String,
    },
    LayerNormal {
        blend: String,
        own: Option<String>,
    },
}

/// One image to make: its key, its source textures and the material's colours.
#[derive(Debug, Clone)]
pub(crate) struct Job {
    pub(crate) key: Key,
    pub(crate) diffuse: Option<Source>,
    pub(crate) normal: Option<Source>,
    pub(crate) specular: Option<Source>,
    pub(crate) explicit_gloss: Vec<Source>,
    pub(crate) tints: Tints,
    pub(crate) layered: Option<Layered>,
}

impl Job {
    pub(crate) const fn layer_maps(&self) -> LayerMaps {
        match self.key {
            Key::LayerNormal { .. } => LayerMaps::Normal,
            Key::LayerFinish { .. } => LayerMaps::Finish,
            _ => LayerMaps::Diffuse,
        }
    }

    /// Returns the file `source` is reported by: its name in the material file, and for a layered
    /// image the image it is the size source of.
    pub(crate) fn file_of(&self, source: &Source) -> String {
        let count = self
            .layered
            .as_ref()
            .map_or(0, |layered| layered.layers.len());
        match &self.key {
            Key::LayerBase { .. } => {
                format!(
                    "{} (layered base colour image of {count} layers)",
                    source.file
                )
            }
            Key::LayerFinish { .. } => format!(
                "{} (layered metallic-roughness image of {count} layers)",
                source.file
            ),
            Key::LayerNormal { .. } => {
                format!("{} (layered normal image of {count} layers)", source.file)
            }
            _ => source.file.clone(),
        }
    }

    /// The role and file reported when the job cannot run at all.
    pub(crate) fn primary(&self) -> Option<(TextureRole, &Source)> {
        match &self.key {
            Key::Base { .. } => self.diffuse.as_ref().map(|s| (TextureRole::Diffuse, s)),
            Key::Normal { .. } => self.normal.as_ref().map(|s| (TextureRole::Normal, s)),
            Key::FactoryBowSpecularColour { .. } => {
                self.specular.as_ref().map(|s| (TextureRole::Specular, s))
            }
            Key::LayerBase { .. } => self
                .layered
                .as_ref()
                .and_then(|layered| layered.sizing(false))
                .map(|s| (TextureRole::Diffuse, s)),
            Key::LayerNormal { .. } => self
                .layered
                .as_ref()
                .and_then(|layered| layered.sizing(true))
                .or(self.normal.as_ref())
                .map(|s| (TextureRole::Normal, s)),
            Key::LayerFinish { .. } => self
                .layered
                .as_ref()
                .and_then(|layered| layered.size_sources(LayerMaps::Finish).next()),
            Key::MetallicRoughness { .. } => self
                .normal
                .as_ref()
                .map(|s| (TextureRole::Normal, s))
                .or_else(|| self.explicit_gloss.first().map(|s| (TextureRole::Gloss, s)))
                .or_else(|| self.specular.as_ref().map(|s| (TextureRole::Specular, s))),
        }
    }
}

/// A PNG image of the GLB.
#[derive(Debug, Clone)]
pub(crate) struct EncodedImage {
    pub(crate) png: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) transfer: DdsTransfer,
    pub(crate) sources: Vec<TextureReadUse>,
}

/// A texture that a job could not use.
#[derive(Debug, Clone)]
pub(crate) struct Failure {
    pub(crate) role: TextureRole,
    pub(crate) file: String,
    pub(crate) reason: String,
}

/// The base colour image of a material.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BaseImage {
    pub(crate) image: usize,
    /// Whether the material's colours are in the image, so that its factor is white.
    pub(crate) tinted: bool,
    /// Whether the image is blended of the layers of a layered material.
    pub(crate) layered: bool,
}

/// The metallic-roughness image of a material and which channels it carries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MetallicRoughnessImage {
    pub(crate) image: usize,
    pub(crate) roughness: bool,
    pub(crate) metallic: bool,
}

/// The images, as indices into [`TextureSet::images`], one material is drawn with.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SlotImages {
    pub(crate) base: Option<BaseImage>,
    pub(crate) normal: Option<usize>,
    pub(crate) specular_colour: Option<usize>,
    pub(crate) metallic_roughness: Option<MetallicRoughnessImage>,
}

/// The PNG images of a GLB, shared between materials, and what each material uses of them.
#[derive(Debug, Default)]
pub(crate) struct TextureSet {
    pub(crate) images: Vec<EncodedImage>,
    pub(crate) image_sources: Vec<Vec<TextureReadUse>>,
    pub(crate) image_materials: Vec<Option<TextureMaterialSource>>,
    pub(crate) slots: BTreeMap<(usize, Slot), SlotImages>,
    pub(crate) fallbacks: Vec<TextureFallback>,
}

/// The jobs of a material: its base colour, normal and metallic-roughness images.
pub(crate) type Wanted = [Option<usize>; 3];

/// The materials of one model instance whose images are wanted.
pub(crate) struct PartSlots<'a> {
    pub(crate) part: usize,
    pub(crate) plan: &'a MaterialPlan<'a>,
    pub(crate) slots: Vec<Slot>,
}

impl TextureSet {
    /// Returns the images of the materials `parts` ask for, loaded at mip level `mip`.
    ///
    /// Every distinct image is made once, whatever number of materials and parts use it; a
    /// texture that cannot be loaded is left out and reported once per material name, and the
    /// material keeps its plain colour for it.
    pub(crate) fn build(parts: &[PartSlots<'_>], mip: u32, loader: &dyn TextureLoader) -> Self {
        let mut jobs: Vec<Job> = Vec::new();
        let mut authors = Vec::new();
        let mut known: HashMap<Key, usize> = HashMap::new();
        let mut wanted: Vec<Wish> = Vec::new();
        for requested in parts {
            for &slot in &requested.slots {
                let material = requested.plan.material(slot);
                let mut add = |job: Job| -> usize {
                    *known.entry(job.key.clone()).or_insert_with(|| {
                        jobs.push(job);
                        authors.push((requested.part, slot, material));
                        jobs.len() - 1
                    })
                };
                let jobs = jobs::jobs_of(
                    material,
                    requested.plan.approximates_factory_bow(slot),
                    &mut add,
                );
                wanted.push(Wish {
                    part: requested.part,
                    slot,
                    name: display_name(material, slot),
                    jobs: jobs.images,
                    specular_colour: jobs.specular_colour,
                    fallback: jobs.fallback,
                    refused: jobs.refused,
                });
            }
        }
        let (jobs, order) = layer_normals_last(jobs, &mut wanted);
        let guards = fallback_guards(&wanted, jobs.len());
        let own: HashMap<String, String> = parts
            .iter()
            .flat_map(|requested| requested.plan.own_scalp_maps())
            .collect();
        let outcomes = if own.is_empty() {
            run_guarded(&jobs, mip, loader, Budget::DEFAULT, &guards)
        } else {
            let loader = WithFallbacks {
                inner: loader,
                own: &own,
            };
            run_guarded(&jobs, mip, &loader, Budget::DEFAULT, &guards)
        };
        let authors: Vec<_> = order
            .into_iter()
            .map(|old| authors.get(old).copied())
            .collect();
        Self::assemble(&wanted, outcomes, &authors)
    }

    /// Returns the images the material of `slot` of the model instance `part` is drawn with.
    pub(crate) fn of(&self, part: usize, slot: Slot) -> SlotImages {
        self.slots.get(&(part, slot)).copied().unwrap_or_default()
    }

    /// Returns the pixels all the images hold.
    pub(crate) fn pixels(&self) -> u64 {
        self.images
            .iter()
            .map(|image| u64::from(image.width) * u64::from(image.height))
            .sum()
    }
}
