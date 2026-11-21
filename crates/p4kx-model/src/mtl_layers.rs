// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::mtl_layer_cache::MAX_LAYER_FILES;
use crate::mtl_number;
use crate::{
    CompanionResolver, LayerCache, Material, MaterialCompanionUse, MaterialLayer, MaterialLibrary,
    TextureSlot,
};

const WHITE: [f32; 3] = [1.0; 3];

/// The surface values of the `.mtl` file a [`MaterialLayer`] names; colours are linear.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LayerSurface {
    /// The layer's diffuse colour, which its tint multiplies.
    pub diffuse: [f32; 3],
    /// The layer's specular colour.
    pub specular: [f32; 3],
    /// The texture slots of the layer's own file, in file order: its diffuse map in `TexSlot1`
    /// and its normal map in `TexSlot2`, with their `TexMod` tiling.
    pub textures: Vec<TextureSlot>,
}

impl MaterialLayer {
    /// Returns the layer's `TintColor`, linear; white when it is missing or malformed.
    #[must_use]
    pub fn tint(&self) -> [f32; 3] {
        self.attribute("TintColor")
            .and_then(mtl_number::vector)
            .unwrap_or(WHITE)
    }

    /// Returns the layer's `GlossMult`, 1 when it is missing or malformed.
    #[must_use]
    pub fn gloss_mult(&self) -> f32 {
        self.attribute("GlossMult")
            .and_then(mtl_number::scalar)
            .unwrap_or(1.0)
    }
}

impl Material {
    /// Returns the `WearBlendBase` of the material's `PublicParams`, the share of the wear layers
    /// in the blend of its layers; 0 when it is missing or malformed.
    #[must_use]
    pub fn wear_blend(&self) -> f32 {
        self.public_params
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("WearBlendBase"))
            .and_then(|(_, value)| mtl_number::scalar(value))
            .unwrap_or(0.0)
    }
}

/// Reads the `.mtl` file of every layer of `library` through `cache` and `companions` and
/// records its surface values in the layer; the paths of the files that are missing or unreadable
/// are listed in [`MaterialLibrary::unresolved_layers`].
pub(crate) fn resolve(
    library: &mut MaterialLibrary,
    companions: &mut dyn CompanionResolver,
    cache: &LayerCache,
) {
    let mut budget = cache.budget();
    let mut unresolved: Vec<String> = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    for (material_index, material) in library.materials.iter_mut().enumerate() {
        for (layer_index, layer) in material.layers.iter_mut().enumerate() {
            let Some(path) = layer
                .attribute("Path")
                .map(str::trim)
                .filter(|p| !p.is_empty())
            else {
                continue;
            };
            let path = path.to_owned();
            let submaterial = layer.attribute("Submtl").map_or_default(str::trim);
            let (source, origin) = cache.surface(&path, submaterial, companions, &mut budget);
            let (surface, source_material) = source.unzip();
            companions.observe_material(
                MaterialCompanionUse::Layer {
                    material: material_index,
                    layer: layer_index,
                    source_material,
                },
                origin.as_ref(),
            );
            if surface.is_none() && unresolved.len() < MAX_LAYER_FILES {
                let normal = format!(
                    "{}|{}",
                    path.replace('\\', "/").to_ascii_lowercase(),
                    submaterial.to_ascii_lowercase()
                );
                if listed.insert(normal) {
                    unresolved.push(path);
                }
            }
            layer.surface = surface;
        }
    }
    library.unresolved_layers = unresolved;
}
