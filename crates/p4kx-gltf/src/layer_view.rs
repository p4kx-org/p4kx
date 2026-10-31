// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::Material;
use rayon::prelude::*;

use crate::layer_jobs::layer_job;
use crate::layer_make::bake_base;
use crate::layer_plan::diffuse_layers;
use crate::texture_cache::{MAX_DECODE_PIXELS, MAX_HELD_PIXELS, SourceStore, paths};
use crate::texture_loader::{LoadedTexture, TextureLoader};
use crate::texture_pixels::Tints;
use crate::texture_plan::Job;

const PARALLEL_IMAGES: usize = 4;
const WHITE: Tints = Tints {
    diffuse: [1.0; 3],
    specular: [1.0; 3],
    decal: None,
};

fn job_of(material: &Material) -> Option<Job> {
    let layered = diffuse_layers(material)?;
    Some(layer_job(&layered, false, None, WHITE))
}

/// Returns a text that is equal for materials whose layered base colour image is the same, or
/// `None` when the textures mode draws no such image for `material`, see `draws_layer_diffuse`.
#[must_use]
pub fn layered_base_key(material: &Material) -> Option<String> {
    diffuse_layers(material).map(|layered| layered.fingerprint(false))
}

/// Blends the base colour image of the layered `material` at mip `mip`, with `loader`.
///
/// This is [`layered_base_textures`] for one material.
#[must_use]
pub fn layered_base_texture(
    material: &Material,
    loader: &dyn TextureLoader,
    mip: u32,
) -> Option<LoadedTexture> {
    layered_base_textures(&[material], loader, mip)
        .into_iter()
        .next()
        .flatten()
}

/// Blends the base colour image of each of the layered `materials` at mip `mip`, with `loader`;
/// the result has one entry per material.
///
/// An image is made of the diffuse maps of the layers and the blend map; the material's colours
/// are in it, so a viewer draws it without a factor. The images are made a few at a time: the
/// distinct textures of those are decoded once, in parallel and without waiting on one another,
/// and those the next ones need stay in memory up to a pixel cap, so the memory held stays within
/// the decode limit of 2^27 pixels plus that cap, however many materials are asked for. An entry
/// is `None` when the material is no layered material with diffuse maps, when its textures, by
/// their header sizes (one whose size is not told counts for the whole limit), would decode more
/// than the limit together, or when none of them can be loaded.
#[must_use]
pub fn layered_base_textures(
    materials: &[&Material],
    loader: &dyn TextureLoader,
    mip: u32,
) -> Vec<Option<LoadedTexture>> {
    let mut results: Vec<Option<LoadedTexture>> = materials.iter().map(|_| None).collect();
    let (positions, jobs): (Vec<usize>, Vec<Job>) = materials
        .iter()
        .enumerate()
        .filter_map(|(at, material)| Some((at, job_of(material)?)))
        .unzip();
    let mut store = SourceStore::new(loader, mip, MAX_HELD_PIXELS);
    let mut done = 0;
    while let Some(rest) = jobs.get(done..).filter(|rest| !rest.is_empty()) {
        let (chunk, after) = rest.split_at(store.group_len(rest, PARALLEL_IMAGES).max(1));
        let next = after.get(..PARALLEL_IMAGES).unwrap_or(after);
        let fits: Vec<bool> = chunk
            .iter()
            .map(|job| store.header_pixels(paths(job)) <= MAX_DECODE_PIXELS)
            .collect();
        let decoded = store.prepare(
            chunk
                .iter()
                .zip(&fits)
                .filter(|(_, fits)| **fits)
                .map(|(job, _)| job),
        );
        let made: Vec<Option<LoadedTexture>> = chunk
            .par_iter()
            .zip(&fits)
            .map(|(job, fits)| {
                if !*fits {
                    return None;
                }
                let image = bake_base(job, &decoded, &mut Vec::new())?.ok()?;
                Some(LoadedTexture::new(image, false))
            })
            .collect();
        store.keep(decoded, next);
        for (offset, texture) in made.into_iter().enumerate() {
            if let Some(slot) = positions
                .get(done + offset)
                .and_then(|position| results.get_mut(*position))
            {
                *slot = texture;
            }
        }
        done += chunk.len();
    }
    results
}
