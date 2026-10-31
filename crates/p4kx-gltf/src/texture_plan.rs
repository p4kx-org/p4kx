// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_model::Material;

use crate::layer_jobs::{fallback_guards, layer_jobs, layer_normals_last};
use crate::layer_plan::Layered;
use crate::materials::{MaterialPlan, Slot, display_name, uses_alpha};
use crate::shading::shading;
use crate::texture_assemble::Wish;
use crate::texture_loader::{TextureLoader, WithFallbacks};
use crate::texture_make::{Budget, run_guarded};
use crate::texture_pixels::Tints;
use crate::texture_report::{TextureFallback, TextureRole};
use crate::texture_roles::{Source, Sources, draws_by_opacity_map, sources_with};

/// The most images one GLB embeds.
pub const MAX_TEXTURE_IMAGES: usize = 256;

/// The most pixels all the images of one GLB hold together.
pub const MAX_TEXTURE_PIXELS: u64 = 1 << 29;

const NEUTRAL: [u8; 3] = [u8::MAX; 3];
const REFUSED: &str = "the path is not a safe texture path";

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
        keep_alpha: bool,
    },
    Normal {
        normal: String,
    },
    MetallicRoughness {
        gloss: Option<String>,
        specular: Option<(String, [u8; 3])>,
    },
    LayerBase {
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
    pub(crate) tints: Tints,
    pub(crate) layered: Option<Layered>,
}

impl Job {
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
            Key::MetallicRoughness { gloss, .. } => {
                if gloss.is_some() {
                    self.normal.as_ref().map(|s| (TextureRole::Normal, s))
                } else {
                    self.specular.as_ref().map(|s| (TextureRole::Specular, s))
                }
            }
        }
    }
}

/// A PNG image of the GLB.
#[derive(Debug, Clone)]
pub(crate) struct EncodedImage {
    pub(crate) png: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
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
    pub(crate) metallic_roughness: Option<MetallicRoughnessImage>,
}

/// The PNG images of a GLB, shared between materials, and what each material uses of them.
#[derive(Debug, Default)]
pub(crate) struct TextureSet {
    pub(crate) images: Vec<EncodedImage>,
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

fn quantise(colour: [f32; 3]) -> [u8; 3] {
    colour.map(|channel| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the channel is between 0 and 1"
        )]
        let byte = (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        byte
    })
}

impl TextureSet {
    /// Returns the images of the materials `parts` ask for, loaded at mip level `mip`.
    ///
    /// Every distinct image is made once, whatever number of materials and parts use it; a
    /// texture that cannot be loaded is left out and reported once per material name, and the
    /// material keeps its plain colour for it.
    pub(crate) fn build(parts: &[PartSlots<'_>], mip: u32, loader: &dyn TextureLoader) -> Self {
        let mut jobs: Vec<Job> = Vec::new();
        let mut known: HashMap<Key, usize> = HashMap::new();
        let mut wanted: Vec<Wish> = Vec::new();
        for requested in parts {
            for &slot in &requested.slots {
                let material = requested.plan.material(slot);
                let mut add = |job: Job| -> usize {
                    *known.entry(job.key.clone()).or_insert_with(|| {
                        jobs.push(job);
                        jobs.len() - 1
                    })
                };
                let (images, fallback, refused) = jobs_of(material, &mut add);
                wanted.push(Wish {
                    part: requested.part,
                    slot,
                    name: display_name(material, slot),
                    jobs: images,
                    fallback,
                    refused,
                });
            }
        }
        let jobs = layer_normals_last(jobs, &mut wanted);
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
        Self::assemble(&wanted, outcomes)
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

fn refusals(refused: &[(TextureRole, String)]) -> Vec<Failure> {
    refused
        .iter()
        .map(|(role, file)| Failure {
            role: *role,
            file: file.clone(),
            reason: REFUSED.to_owned(),
        })
        .collect()
}

fn tints_of(material: &Material) -> Tints {
    let shade = shading(material);
    Tints {
        diffuse: shade.diffuse.map(|channel| channel.clamp(0.0, 1.0)),
        specular: shade.specular.map(|channel| channel.clamp(0.0, 1.0)),
    }
}

/// Returns the job of the base colour image of the diffuse map `diffuse` of `material`, with the
/// specular map of `found`, if any.
fn base_job(
    material: &Material,
    (found, diffuse): (&Sources, &Source),
    specular: Option<(String, [u8; 3])>,
    tints: Tints,
) -> Job {
    let tint = if specular.is_some() {
        quantise(tints.diffuse)
    } else {
        NEUTRAL
    };
    Job {
        key: Key::Base {
            diffuse: diffuse.identity(),
            specular,
            tint,
            keep_alpha: diffuse.opacity || uses_alpha(material),
        },
        diffuse: Some(diffuse.clone()),
        normal: None,
        specular: found.specular.clone(),
        tints,
        layered: None,
    }
}

/// Registers the jobs for the images of `material` with `add`, which returns a job's number, and
/// returns the jobs with the textures refused for their unsafe paths.
fn jobs_of(
    material: &Material,
    add: &mut impl FnMut(Job) -> usize,
) -> (Wanted, Option<usize>, Vec<Failure>) {
    let mut found = sources_with(material, draws_by_opacity_map(material));
    if found.diffuse.as_ref().is_none_or(|diffuse| diffuse.opacity) {
        found.specular = None;
    }
    let mut refused = refusals(&found.refused);
    let tints = tints_of(material);
    let specular_key = found
        .specular
        .as_ref()
        .map(|specular| (specular.identity(), quantise(tints.specular)));
    let (layer_base, layer_normal, layer_refused) = layer_jobs(material, &found, tints);
    refused.extend(refusals(&layer_refused));
    let base = match (&found.diffuse, layer_base) {
        (Some(diffuse), _) => Some(add(base_job(
            material,
            (&found, diffuse),
            specular_key.clone(),
            tints,
        ))),
        (None, layered) => layered.map(&mut *add),
    };
    let plain_normal = found.normal.as_ref().map(|normal| {
        add(Job {
            key: Key::Normal {
                normal: normal.identity(),
            },
            diffuse: None,
            normal: found.normal.clone(),
            specular: None,
            tints,
            layered: None,
        })
    });
    let (normal, fallback) = layer_normal.map_or((plain_normal, None), |layered| {
        (Some(add(layered)), plain_normal)
    });
    let gloss = found.normal.as_ref().filter(|normal| normal.gloss);
    let packed = (gloss.is_some() || specular_key.is_some()).then(|| {
        add(Job {
            key: Key::MetallicRoughness {
                gloss: gloss.map(Source::identity),
                specular: specular_key.clone(),
            },
            diffuse: None,
            normal: gloss.cloned(),
            specular: found.specular.clone(),
            tints,
            layered: None,
        })
    });
    ([base, normal, packed], fallback, refused)
}
