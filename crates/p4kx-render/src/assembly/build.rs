// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::Assembly;
use p4kx_extraction::model::Material;
use p4kx_extraction::model::Model;
use p4kx_extraction::{LoadedTexture, TextureLoader, layered_base_textures};
use p4kx_settings::ModelMaterials;
use rayon::prelude::*;
use thiserror::Error;

use super::flatten::{Range, flatten};
use super::pieces::pieces_of;
use super::surface::{SceneSurfaces, surfaces_of};
use super::{
    AssemblyScene, Left, MAX_VIEW_TEXTURE_BYTES, MAX_VIEW_TRIANGLES, MAX_VIEW_VERTICES,
    MaterialSuppression, VIEW_TEXTURE_MIP, ViewTexture, chain,
};
use crate::scene::{Bounds, next_serial};

const WAVE: usize = 64;
const TEXTURE_WAVE: usize = 16;
const MAX_TEXTURE_SIDE: u32 = 4096;

/// Why an Assembly cannot be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BuildError {
    /// No part has triangles the viewer can show.
    #[error("the Assembly has no triangles to show")]
    Empty,
    /// Building was stopped.
    #[error("building the view was stopped")]
    Cancelled,
}

/// How far building the scene has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Models flattened of the distinct ones.
    Flattening {
        /// How many are done.
        done: usize,
        /// How many there are.
        total: usize,
    },
    /// Textures read of the distinct ones.
    Textures {
        /// How many are done.
        done: usize,
        /// How many there are.
        total: usize,
    },
}

/// What the scene is drawn with.
#[derive(Clone, Copy)]
pub struct BuildOptions<'a> {
    /// Plain colours, or the diffuse textures over them.
    pub materials: ModelMaterials,
    /// Where the textures come from; without it the materials keep their plain colours.
    pub textures: Option<&'a dyn TextureLoader>,
}

/// The ways to stop building and to watch it.
#[derive(Clone, Copy)]
pub struct Hooks<'a> {
    /// Returns whether building should stop.
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
    /// Called as building advances, from any thread.
    pub progress: &'a (dyn Fn(Progress) + Sync),
}

pub(super) struct SharedMesh {
    pub(super) ranges: Vec<Range>,
    pub(super) bounds: Bounds,
    pub(super) triangles: usize,
}

fn address<T>(value: &T) -> usize {
    std::ptr::from_ref(value) as usize
}

impl AssemblyScene {
    /// Flattens `assembly`: each distinct model once, in parallel, then the parts as instances
    /// of those meshes with the surface each material range resolves to.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Cancelled`] when `hooks` ask to stop and [`BuildError::Empty`] when
    /// nothing can be shown.
    pub fn build(
        assembly: &Assembly,
        options: &BuildOptions<'_>,
        hooks: &Hooks<'_>,
    ) -> Result<Self, BuildError> {
        Self::build_inner(assembly, options, hooks, None)
    }

    /// Builds the scene and retains actual selected material suppressions.
    ///
    /// # Errors
    /// Returns a cancellation or empty-scene error.
    pub fn build_observed(
        assembly: &Assembly,
        options: &BuildOptions<'_>,
        hooks: &Hooks<'_>,
    ) -> Result<Self, BuildError> {
        Self::build_inner(assembly, options, hooks, Some(Vec::new()))
    }

    fn build_inner(
        assembly: &Assembly,
        options: &BuildOptions<'_>,
        hooks: &Hooks<'_>,
        material_suppressions: Option<Vec<MaterialSuppression>>,
    ) -> Result<Self, BuildError> {
        let pieces = pieces_of(assembly);
        let mut ids: HashMap<usize, usize> = HashMap::new();
        let mut distinct: Vec<&Model> = Vec::new();
        let model_of: Vec<Option<usize>> = pieces
            .iter()
            .map(|piece| {
                piece.model.map(|model| {
                    *ids.entry(address(model)).or_insert_with(|| {
                        distinct.push(model);
                        distinct.len() - 1
                    })
                })
            })
            .collect();
        let mut scene = Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            groups: Vec::new(),
            items: Vec::new(),
            draws: Vec::new(),
            parts: Vec::new(),
            material_suppressions,
            textures: Vec::new(),
            meshes: 0,
            bounds: Bounds::EMPTY,
            left: Left::default(),
            serial: next_serial(),
        };
        let meshes = scene.merge_meshes(&distinct, assembly.lod(), hooks)?;
        scene.meshes = meshes.iter().flatten().count();
        let wanted = surfaces_of(&pieces, &model_of, &meshes, options.materials);
        let textures = scene.read_textures(&wanted, options, hooks)?;
        scene.place(&pieces, (&model_of, &meshes), (&wanted.surfaces, &textures));
        if scene.draws.is_empty() || !scene.bounds.is_valid() {
            return Err(BuildError::Empty);
        }
        Ok(scene)
    }

    fn merge_meshes(
        &mut self,
        distinct: &[&Model],
        lod: u32,
        hooks: &Hooks<'_>,
    ) -> Result<Vec<Option<SharedMesh>>, BuildError> {
        let mut meshes = Vec::with_capacity(distinct.len());
        for wave in distinct.chunks(WAVE) {
            if (hooks.cancelled)() {
                return Err(BuildError::Cancelled);
            }
            let flats: Vec<_> = wave.par_iter().map(|model| flatten(model, lod)).collect();
            for flat in flats {
                let mesh = flat.and_then(|flat| {
                    let vertices = self.vertices.len().checked_add(flat.vertices.len())?;
                    let triangles = (self.indices.len() + flat.indices.len()) / 3;
                    if vertices > MAX_VIEW_VERTICES || triangles > MAX_VIEW_TRIANGLES {
                        return None;
                    }
                    let base = u32::try_from(self.vertices.len()).ok()?;
                    let first = u32::try_from(self.indices.len()).ok()?;
                    let count = flat.indices.len() / 3;
                    self.indices
                        .extend(flat.indices.iter().map(|index| index.saturating_add(base)));
                    self.vertices.extend(flat.vertices);
                    Some(SharedMesh {
                        ranges: flat
                            .ranges
                            .into_iter()
                            .map(|range| Range {
                                first_index: range.first_index + first,
                                ..range
                            })
                            .collect(),
                        bounds: flat.bounds,
                        triangles: count,
                    })
                });
                meshes.push(mesh);
            }
            (hooks.progress)(Progress::Flattening {
                done: meshes.len(),
                total: distinct.len(),
            });
        }
        Ok(meshes)
    }

    fn read_textures(
        &mut self,
        SceneSurfaces {
            wanted, layered, ..
        }: &SceneSurfaces,
        options: &BuildOptions<'_>,
        hooks: &Hooks<'_>,
    ) -> Result<HashMap<String, u32>, BuildError> {
        let mut kept = HashMap::new();
        let Some(loader) = options.textures.filter(|_| !wanted.is_empty()) else {
            self.left.textures = wanted.len();
            return Ok(kept);
        };
        let mut bytes = 0_usize;
        for (done, wave) in wanted.chunks(TEXTURE_WAVE).enumerate() {
            if (hooks.cancelled)() {
                return Err(BuildError::Cancelled);
            }
            let images = load_wave(wave, layered, loader);
            for (path, texture) in wave.iter().zip(images) {
                let image = texture.map(|texture| texture.image).filter(|image| {
                    image.width() > 0
                        && image.height() > 0
                        && image.width() <= MAX_TEXTURE_SIDE
                        && image.height() <= MAX_TEXTURE_SIDE
                });
                let Some(image) = image else {
                    self.left.textures += 1;
                    continue;
                };
                let size = image.pixels().len();
                if bytes.saturating_add(size) > MAX_VIEW_TEXTURE_BYTES {
                    self.left.textures += 1;
                    continue;
                }
                bytes += size;
                kept.insert(
                    path.clone(),
                    u32::try_from(self.textures.len()).unwrap_or(0),
                );
                let mips = chain(image.width(), image.height(), image.pixels());
                self.textures.push(ViewTexture::new(
                    image.width(),
                    image.height(),
                    image.into_pixels(),
                    mips,
                ));
            }
            (hooks.progress)(Progress::Textures {
                done: ((done + 1) * TEXTURE_WAVE).min(wanted.len()),
                total: wanted.len(),
            });
        }
        Ok(kept)
    }
}

/// Loads the textures of `wave`, a few of the layered materials' images blended together under one
/// decode budget and the others in parallel; an entry is `None` when its texture cannot be made.
fn load_wave(
    wave: &[String],
    layered: &HashMap<String, Material>,
    loader: &dyn TextureLoader,
) -> Vec<Option<LoadedTexture>> {
    let mut images: Vec<Option<LoadedTexture>> = wave
        .par_iter()
        .map(|path| {
            if layered.contains_key(path) {
                None
            } else {
                loader.load(path, VIEW_TEXTURE_MIP).ok()
            }
        })
        .collect();
    let blended: Vec<(usize, &Material)> = wave
        .iter()
        .enumerate()
        .filter_map(|(at, path)| Some((at, layered.get(path)?)))
        .collect();
    let materials: Vec<&Material> = blended.iter().map(|(_, material)| *material).collect();
    for ((at, _), texture) in
        blended
            .iter()
            .zip(layered_base_textures(&materials, loader, VIEW_TEXTURE_MIP))
    {
        if let Some(slot) = images.get_mut(*at) {
            *slot = texture;
        }
    }
    images
}
