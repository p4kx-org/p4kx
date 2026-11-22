// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use p4kx_model::{
    LayerSurface, Material, MaterialLayer, TextureRole as MaterialTextureRole, TextureSlot,
};

use super::{LayerMaps, LayerPlan, Layered};
use crate::texture_report::TextureRole;
use crate::texture_roles::{Source, canonical};

const MAX_TILING: f32 = 4096.0;
const TEXTURE_MODIFIER: &str = "TexMod";
const LAYER_SPECULAR_SLOT: &str = "TexSlot6";
const SPECULAR_MAPS_FLAG: &str = "USE_SPECULAR_MAPS";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerMap {
    Diffuse,
    Normal,
    Specular,
}

impl LayerMap {
    pub(crate) const fn role(self) -> TextureRole {
        match self {
            Self::Diffuse => TextureRole::Diffuse,
            Self::Normal => TextureRole::Normal,
            Self::Specular => TextureRole::Specular,
        }
    }

    pub(crate) const fn source(self, layer: &LayerPlan) -> Option<&(Source, [f32; 2])> {
        match self {
            Self::Diffuse => layer.diffuse.as_ref(),
            Self::Normal => layer.normal.as_ref(),
            Self::Specular => layer.specular_map.as_ref(),
        }
    }
}

impl LayerPlan {
    pub(super) fn append_specular_fingerprint(&self, key: &mut String) {
        if let Some((source, repeats)) = &self.specular_map {
            let _ = write!(
                key,
                "|specular={}:{:x?}:{:x?}",
                source.identity(),
                repeats.map(f32::to_bits),
                self.specular.map(f32::to_bits),
            );
        }
    }
}

impl Layered {
    pub(crate) fn required_paths(&self, maps: LayerMaps) -> impl Iterator<Item = &str> {
        self.size_sources(maps)
            .filter(|(role, _)| *role != TextureRole::Specular)
            .map(|(_, source)| source.path.as_str())
    }

    pub(super) fn append_decode_dependencies(&self, key: &mut String) {
        append_exact_paths(
            key,
            "decode-diffuse",
            self.required_paths(LayerMaps::Diffuse),
        );
        append_exact_paths(key, "decode-finish", self.required_paths(LayerMaps::Finish));
        append_exact_paths(
            key,
            "decode-specular",
            self.specular_sources().map(|source| source.path.as_str()),
        );
    }
}

fn append_exact_paths<'path>(
    key: &mut String,
    label: &str,
    paths: impl Iterator<Item = &'path str>,
) {
    for path in paths {
        let _ = write!(key, "|{label}={}:{}", path.len(), path);
    }
}

fn number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.min(MAX_TILING))
}

fn modifier<'slot>(textures: impl Iterator<Item = &'slot TextureSlot>, name: &str) -> f32 {
    textures
        .flat_map(|texture| &texture.children)
        .filter(|child| child.name.eq_ignore_ascii_case(TEXTURE_MODIFIER))
        .flat_map(|child| &child.attributes)
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| number(value))
        .unwrap_or(1.0)
}

fn repeats_of<'slot>(
    layer: &MaterialLayer,
    textures: impl Iterator<Item = &'slot TextureSlot> + Clone,
) -> [f32; 2] {
    let all = layer.attribute("UVTiling").and_then(number).unwrap_or(1.0);
    let tiling = [
        modifier(textures.clone(), "TileU"),
        modifier(textures, "TileV"),
    ];
    tiling.map(|value| (value * all).min(MAX_TILING))
}

pub(super) fn repeats(layer: &MaterialLayer, textures: &[TextureSlot], slot: &str) -> [f32; 2] {
    repeats_of(layer, textures.iter().filter(|texture| texture.map == slot))
}

pub(super) fn selected_specular(
    material: &Material,
    layer: &MaterialLayer,
    surface: &LayerSurface,
    refused: &mut Vec<(TextureRole, String)>,
) -> Option<(Source, [f32; 2])> {
    if !material.shader.eq_ignore_ascii_case("HardSurface")
        || !surface.is_layer_shader()
        || layer.palette_entry.is_some()
        || !material
            .string_gen_mask
            .split('%')
            .skip(1)
            .any(|flag| flag == SPECULAR_MAPS_FLAG)
    {
        return None;
    }
    let slot = surface.textures.iter().find(|slot| {
        slot.map == LAYER_SPECULAR_SLOT
            && !slot.file.trim_start().starts_with('$')
            && MaterialTextureRole::of(&slot.map, &slot.file) == MaterialTextureRole::Specular
    })?;
    let Some(path) = canonical(&slot.file) else {
        refused.push((TextureRole::Specular, slot.file.clone()));
        return None;
    };
    let source = Source {
        path,
        file: slot.file.clone(),
        gloss: false,
        opacity: false,
    };
    Some((source, repeats_of(layer, std::iter::once(slot))))
}
