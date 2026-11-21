// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use p4kx_model::{
    DIFFUSE_SLOT, Material, MaterialLayer, TextureSlot, texture_suffix_lowercase as suffix,
};

use crate::shading::{Shaded, shaded_layers};
use crate::surface_finish::SurfaceFinish;
use crate::texture_report::TextureRole;
use crate::texture_roles::{Source, canonical, layer_gloss_sources, sources_of};

/// The most layers one baked image blends; the others keep out of it.
pub(crate) const MAX_BAKED_LAYERS: usize = 8;

/// The layers at the front of a material that its blend mask chooses between: the first by its
/// red, the second by its green and the third by its blue channel, each above the ones after it,
/// and the fourth where no channel is set.
pub(crate) const BASE_LAYERS: usize = 4;

const MAX_TILING: f32 = 4096.0;
const MASK_SUFFIX: &str = "blend";
const TEXTURE_MODIFIER: &str = "TexMod";

/// One layer of a layered material as the image bake sees it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayerPlan {
    pub(crate) gloss: Vec<(Source, [f32; 2])>,
    pub(crate) finish: Option<SurfaceFinish>,
    pub(crate) roughness: f32,
    /// The layer's position in the material's `MatLayers`.
    pub(crate) at: usize,
    /// The share the layer has without a blend mask: by its position and the wear share.
    pub(crate) share: f32,
    /// The layer's diffuse map and how often it repeats over the UVs.
    pub(crate) diffuse: Option<(Source, [f32; 2])>,
    /// The layer's normal map and how often it repeats over the UVs.
    pub(crate) normal: Option<(Source, [f32; 2])>,
    /// The layer's diffuse colour times its tint, which the diffuse map multiplies.
    pub(crate) colour: [f32; 3],
    /// The layer's specular colour, which its colour moves towards by its metalness.
    pub(crate) specular: [f32; 3],
    /// How metallic the layer is: its colour moves towards its specular colour by this much.
    pub(crate) metal: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerMaps {
    Diffuse,
    Normal,
    Smoothness,
}

/// What the images of a layered material are blended of.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layered {
    pub(crate) layers: Vec<LayerPlan>,
    /// The blend mask of the material: its red, green and blue channel choose the first three of
    /// the [`BASE_LAYERS`] in that order of precedence, the rest the fourth.
    pub(crate) mask: Option<Source>,
    /// The texture files of the blend mask and the layers that have no safe path, with their role.
    pub(crate) refused: Vec<(TextureRole, String)>,
}

fn number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.min(MAX_TILING))
}

fn modifier(textures: &[TextureSlot], slot: &str) -> [f32; 2] {
    let attribute = |name: &str| {
        textures
            .iter()
            .filter(|texture| texture.map == slot)
            .flat_map(|texture| &texture.children)
            .filter(|child| child.name.eq_ignore_ascii_case(TEXTURE_MODIFIER))
            .flat_map(|child| &child.attributes)
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| number(value))
            .unwrap_or(1.0)
    };
    [attribute("TileU"), attribute("TileV")]
}

fn repeats(layer: &MaterialLayer, textures: &[TextureSlot], slot: &str) -> [f32; 2] {
    let all = layer.attribute("UVTiling").and_then(number).unwrap_or(1.0);
    modifier(textures, slot).map(|tile| (tile * all).min(MAX_TILING))
}

/// Returns the blend mask of `material`: its texture whose name ends in `_blend`.
fn mask_of(material: &Material, refused: &mut Vec<(TextureRole, String)>) -> Option<Source> {
    let file = material.textures.iter().find_map(|texture| {
        let file = texture.file.as_str();
        (!file.trim_start().starts_with('$') && suffix(file) == MASK_SUFFIX).then_some(file)
    })?;
    let source = canonical(file).map(|path| Source {
        path,
        file: file.to_owned(),
        gloss: false,
        opacity: false,
    });
    if source.is_none() {
        refused.push((TextureRole::Diffuse, file.to_owned()));
    }
    source
}

fn plan_of(
    material: &Material,
    shaded: &Shaded,
    refused: &mut Vec<(TextureRole, String)>,
) -> Option<LayerPlan> {
    let layer = material.layers.get(shaded.at)?;
    let surface = layer.surface.as_ref()?;
    let mut found = sources_of(&surface.textures, false);
    refused.append(&mut found.refused);
    let tiled = |source: Option<Source>, slot: &str| {
        source.map(|source| {
            let tiling = repeats(layer, &surface.textures, slot);
            (source, tiling)
        })
    };
    let normal_slot = surface
        .textures
        .iter()
        .find(|texture| {
            found
                .normal
                .as_ref()
                .is_some_and(|n| n.file == texture.file)
        })
        .map_or(DIFFUSE_SLOT, |texture| texture.map.as_str());
    let gloss = (if shaded.layer.finish.is_some() {
        layer_gloss_sources(material, &surface.textures, refused)
    } else {
        Vec::new()
    })
    .into_iter()
    .map(|source| {
        let tiling = repeats(
            layer,
            &surface.textures,
            surface
                .textures
                .iter()
                .find(|slot| slot.file == source.file)
                .map_or(DIFFUSE_SLOT, |slot| slot.map.as_str()),
        );
        (source, tiling)
    })
    .collect();
    Some(LayerPlan {
        gloss,
        finish: shaded.layer.finish,
        roughness: shaded.layer.roughness,
        at: shaded.at,
        share: shaded.share,
        diffuse: tiled(found.diffuse, DIFFUSE_SLOT),
        normal: tiled(found.normal, normal_slot),
        colour: shaded.layer.diffuse,
        specular: shaded.layer.specular,
        metal: shaded.layer.metal,
    })
}

/// Returns what the images of `material` are blended of: its layers, whose files were read, that
/// the blend mask or the wear share can give a share, with their textures.
///
/// `None` when the material has no such layer or none of them has a diffuse or a normal map.
pub(crate) fn layered_of(material: &Material) -> Option<Layered> {
    let shaded = shaded_layers(material)?;
    let mut refused = Vec::new();
    let mask = mask_of(material, &mut refused);
    let mut layers: Vec<LayerPlan> = shaded
        .iter()
        .filter(|layer| layer.share > 0.0 || (mask.is_some() && layer.at < BASE_LAYERS))
        .filter_map(|layer| plan_of(material, layer, &mut refused))
        .collect();
    layers.truncate(MAX_BAKED_LAYERS);
    let mapped = layers.iter().any(|layer| {
        layer.diffuse.is_some()
            || layer.normal.is_some()
            || layer.finish.is_some_and(|_| !layer.gloss.is_empty())
    });
    mapped.then_some(Layered {
        layers,
        mask,
        refused,
    })
}

/// Returns whether the textures mode draws the layers' diffuse maps of `material`: it has no
/// diffuse map of its own and a layer whose file was read has one.
#[must_use]
pub fn draws_layer_diffuse(material: &Material) -> bool {
    diffuse_layers(material).is_some()
}

/// Returns the layers of `material` whose diffuse maps its base colour image is blended of.
pub(crate) fn diffuse_layers(material: &Material) -> Option<Layered> {
    if sources_of(&material.textures, false).diffuse.is_some() {
        return None;
    }
    layered_of(material).filter(Layered::has_diffuse)
}

impl Layered {
    /// Returns whether a layer has a diffuse map.
    pub(crate) fn has_diffuse(&self) -> bool {
        self.layers.iter().any(|layer| layer.diffuse.is_some())
    }

    /// Returns whether a layer has a normal map.
    pub(crate) fn has_normal(&self) -> bool {
        self.layers.iter().any(|layer| layer.normal.is_some())
    }

    /// Returns the first source of the maps `normal` picks, which sizes the image when the
    /// mask does not.
    pub(crate) fn first(&self, normal: bool) -> Option<&Source> {
        self.layers.iter().find_map(|layer| {
            let map = if normal {
                &layer.normal
            } else {
                &layer.diffuse
            };
            map.as_ref().map(|(source, _)| source)
        })
    }

    pub(crate) fn size_sources(&self, maps: LayerMaps) -> impl Iterator<Item = &Source> {
        self.mask
            .iter()
            .chain(self.layers.iter().flat_map(move |layer| {
                let primary = match maps {
                    LayerMaps::Diffuse => &layer.diffuse,
                    _ => &layer.normal,
                };
                primary
                    .iter()
                    .filter(move |(source, _)| maps != LayerMaps::Smoothness || source.gloss)
                    .map(|(source, _)| source)
                    .chain(
                        layer
                            .gloss
                            .iter()
                            .filter(move |_| maps == LayerMaps::Smoothness)
                            .map(|(source, _)| source),
                    )
            }))
    }

    pub(crate) fn sizing(&self, normal: bool) -> Option<&Source> {
        self.mask.as_ref().or_else(|| self.first(normal))
    }

    pub(crate) fn paths(&self, maps: LayerMaps) -> impl Iterator<Item = &str> {
        self.size_sources(maps).map(|source| source.path.as_str())
    }

    pub(crate) fn has_sampled_finish(&self) -> bool {
        self.layers.iter().any(|layer| layer.finish.is_some())
            && self.layers.iter().any(|layer| {
                layer
                    .normal
                    .as_ref()
                    .is_some_and(|(source, _)| source.gloss)
                    || !layer.gloss.is_empty()
            })
    }

    pub(crate) fn finish_fingerprint(&self) -> String {
        let mut text = self.fingerprint(true);
        for layer in &self.layers {
            let _ = write!(
                text,
                "|finish={:?}:{:x}",
                layer.finish,
                layer.roughness.to_bits()
            );
            for (source, tiling) in &layer.gloss {
                let _ = write!(
                    text,
                    "|gloss={}:{:x}:{:x}",
                    source.identity(),
                    tiling[0].to_bits(),
                    tiling[1].to_bits()
                );
            }
        }
        text
    }

    /// Returns a text that is equal for equal blends of the diffuse or normal maps; the key of
    /// the image.
    pub(crate) fn fingerprint(&self, normal: bool) -> String {
        let mut text = String::new();
        if let Some(mask) = &self.mask {
            let _ = write!(text, "mask={}", mask.identity());
        }
        for layer in &self.layers {
            let map = if normal {
                &layer.normal
            } else {
                &layer.diffuse
            };
            let (path, tiling) = map
                .as_ref()
                .map_or((String::new(), [0.0; 2]), |(source, tiling)| {
                    (source.identity(), *tiling)
                });
            let _ = write!(
                text,
                "|{}:{}:{:x}:{:x}:{:x}",
                layer.at,
                path,
                tiling[0].to_bits(),
                tiling[1].to_bits(),
                layer.share.to_bits(),
            );
            if !normal {
                let _ = write!(
                    text,
                    ":{:x?}:{:x?}:{:x}",
                    layer.colour.map(f32::to_bits),
                    layer.specular.map(f32::to_bits),
                    layer.metal.to_bits(),
                );
            }
        }
        text
    }

    /// Writes the share each layer has at a texel into `shares`, one per layer in order: the
    /// layers' shares by position, or with a mask value `mask` (red, green and blue as 0 to 1)
    /// the base layers by the mask, red over green over blue over none, and the rest by position.
    /// A base layer the material lacks is skipped, and the last base layer present takes what no
    /// channel of the layers before it claims.
    pub(crate) fn shares(&self, mask: Option<[f32; 3]>, shares: &mut [f32; MAX_BAKED_LAYERS]) {
        shares.fill(0.0);
        let positional = |layer: &LayerPlan| layer.share;
        let Some(mask) = mask else {
            for (share, layer) in shares.iter_mut().zip(&self.layers) {
                *share = positional(layer);
            }
            return;
        };
        let [red, green, blue] = mask.map(|channel| channel.clamp(0.0, 1.0));
        let channels = [red, green, blue, 1.0];
        let last_base = self.layers.iter().rposition(|layer| layer.at < BASE_LAYERS);
        let Some(last_base) = last_base else {
            for (share, layer) in shares.iter_mut().zip(&self.layers) {
                *share = positional(layer);
            }
            return;
        };
        let rest: f32 = self
            .layers
            .iter()
            .filter(|layer| layer.at >= BASE_LAYERS)
            .map(positional)
            .sum();
        let mut remaining = (1.0 - rest).max(0.0);
        for (index, (share, layer)) in shares.iter_mut().zip(&self.layers).enumerate() {
            if layer.at >= BASE_LAYERS {
                *share = positional(layer);
                continue;
            }
            let channel = if index == last_base {
                1.0
            } else {
                channels.get(layer.at).copied().unwrap_or(0.0)
            };
            *share = channel * remaining;
            remaining *= 1.0 - channel;
        }
    }
}

#[cfg(test)]
mod tests;
