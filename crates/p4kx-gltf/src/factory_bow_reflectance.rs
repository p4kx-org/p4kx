// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{
    LayerSurface, Material, PaletteSlot, TextureRole as MaterialTextureRole, material_library_path,
};
use p4kx_texture::{DdsTransfer, RgbaImage};
use sha2::{Digest, Sha256};

use crate::material_alpha::uses_alpha;
use crate::texture_cache::Decoded;
use crate::texture_make::{self, Made, Outcome};
use crate::texture_origins::{TextureComponents, TexturePaletteUse};
use crate::texture_pixels::{self, RGBA, Tints};
use crate::texture_plan::{Failure, Job, Key};
use crate::texture_report::TextureRole;
use crate::texture_roles::{Source, canonical};

pub(crate) const EXTENSION: &str = "KHR_materials_specular";
pub(crate) const SLOT: usize = 6;
const FACTORY_FINGERPRINTS: [[u8; 32]; 2] = [
    [
        0x8a, 0x10, 0xa1, 0xba, 0x14, 0xc1, 0x7a, 0x91, 0x9d, 0x86, 0xc1, 0xb3, 0xe3, 0x81, 0x1b,
        0xf4, 0x57, 0x51, 0x99, 0x7c, 0x27, 0x1f, 0x1b, 0x29, 0x52, 0x6c, 0xb5, 0x58, 0xfc, 0xdc,
        0x7c, 0x74,
    ],
    [
        0xcb, 0x73, 0x46, 0x83, 0xfa, 0x6d, 0x73, 0xeb, 0x17, 0x49, 0x15, 0xc3, 0x23, 0x79, 0x21,
        0x90, 0x56, 0x78, 0xfd, 0xcf, 0x5e, 0xd8, 0x23, 0xf5, 0x86, 0x8a, 0xae, 0x34, 0x4d, 0x72,
        0xd9, 0xe5,
    ],
];
#[cfg(test)]
const TEST_FINGERPRINT: [u8; 32] = [
    0x57, 0x7a, 0x18, 0x3c, 0xb9, 0xec, 0x85, 0x14, 0xf6, 0x23, 0x1f, 0xb1, 0xc1, 0xe9, 0x32, 0xfb,
    0xda, 0x93, 0xcc, 0x67, 0x4c, 0x23, 0x5e, 0x3d, 0x64, 0xb1, 0xd1, 0xe6, 0x29, 0x06, 0x16, 0x1c,
];
const BLENDER_LEVEL_PER_REFLECTANCE: f32 = 12.5;
// Neutral specularColor 1 gives F0=0.04; Blender IOR Level 1 gives F0=0.08.
const COLOUR_PER_LEVEL: f32 = 2.0;
const FOUNDATION: usize = 0;

#[cfg(test)]
mod tests;

pub(crate) fn matches(library: &str, palette: &str) -> bool {
    let Some(fingerprint) = fingerprint(library, palette) else {
        return false;
    };
    #[cfg(test)]
    if fingerprint == TEST_FINGERPRINT {
        return true;
    }
    FACTORY_FINGERPRINTS.contains(&fingerprint)
}

fn fingerprint(library: &str, palette: &str) -> Option<[u8; 32]> {
    let mut library = material_library_path(library)?;
    library.make_ascii_lowercase();
    let mut hash = Sha256::new();
    // The little-endian length frames the normalized path before the exact palette bytes.
    hash.update((library.len() as u64).to_le_bytes());
    hash.update(library.as_bytes());
    hash.update(palette.as_bytes());
    Some(hash.finalize().into())
}
pub(crate) fn job(material: &Material, tints: Tints) -> Option<Job> {
    let (source, scale) = source_and_scale(material)?;
    Some(Job {
        key: Key::FactoryBowSpecularColour {
            source: source.identity(),
            level_scale: scale.to_bits(),
        },
        diffuse: None,
        normal: None,
        specular: Some(source),
        explicit_gloss: Vec::new(),
        tints,
        layered: None,
    })
}

fn source_and_scale(material: &Material) -> Option<(Source, f32)> {
    let [foundation, wear] = material.layers.as_slice() else {
        return None;
    };
    if !material.shader.eq_ignore_ascii_case("HardSurface")
        || !material.textures.is_empty()
        || uses_alpha(material)
        || foundation.palette_slot() != Some(PaletteSlot::Secondary)
        || wear.palette_slot() != Some(PaletteSlot::Secondary)
        || !unit_tiling(foundation.attribute("UVTiling"))
    {
        return None;
    }
    let entry = foundation.palette_entry?;
    if wear.palette_entry != Some(entry)
        || !entry
            .specular
            .iter()
            .all(|channel| (0.0..=1.0).contains(channel))
        || texture_pixels::metal_share(entry.specular) > 0.0
    {
        return None;
    }
    let layer = foundation
        .surface
        .as_ref()
        .filter(|layer| layer.is_layer_shader())?;
    if !wear
        .surface
        .as_ref()
        .is_some_and(LayerSurface::is_layer_shader)
        || layer.textures.iter().any(|slot| {
            MaterialTextureRole::of(&slot.map, &slot.file) == MaterialTextureRole::Diffuse
        })
    {
        return None;
    }
    let mut specular = layer.textures.iter().filter(|slot| {
        MaterialTextureRole::of(&slot.map, &slot.file) == MaterialTextureRole::Specular
    });
    let (slot, alias) = (specular.next()?, specular.next()?);
    let path = canonical(&slot.file)?;
    if slot.map != "TexSlot6"
        || alias.map != "TexSlot8"
        || !slot.children.is_empty()
        || !alias.children.is_empty()
        || canonical(&alias.file).as_deref() != Some(path.as_str())
        || specular.next().is_some()
    {
        return None;
    }
    let scale = texture_pixels::luminance(entry.specular) * BLENDER_LEVEL_PER_REFLECTANCE;
    let source = Source {
        path,
        file: slot.file.clone(),
        gloss: false,
        opacity: false,
    };
    Some((source, scale))
}

fn unit_tiling(tiling: Option<&str>) -> bool {
    tiling.is_none_or(|value| {
        value.trim().parse::<f32>().ok().map(f32::to_bits) == Some(1.0_f32.to_bits())
    })
}

pub(crate) fn make(
    job: &Job,
    level_scale: u32,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let loaded = texture_make::load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    )?;
    let contribution = loaded.colour_contribution(TextureComponents::Color);
    if contribution.fallback {
        return Some(Err(
            "the factory bow specular colour requires the original texture".to_owned(),
        ));
    }
    let transfer = loaded.decoded_transfer();
    if !matches!(transfer, DdsTransfer::Srgb | DdsTransfer::Linear) {
        return Some(Err(
            "the factory bow specular colour requires a declared RGB transfer".to_owned(),
        ));
    }
    let image = loaded.colour_image().map_err(|error| error.to_string());
    let decode = texture_pixels::colour_decoder(transfer);
    let baked = image.and_then(|image| bake(&image, decode, f32::from_bits(level_scale)));
    let encoded = baked.and_then(|image| texture_make::encode(&image, DdsTransfer::Srgb));
    if encoded.is_ok() {
        outcome.texture_sources.push(contribution);
        outcome.material_layers.push(FOUNDATION);
        outcome.palette_use = TexturePaletteUse::Layers;
    }
    Some(encoded)
}

fn bake(source: &RgbaImage, decode: fn(u8) -> f32, level_scale: f32) -> Result<RgbaImage, String> {
    let mut pixels = Vec::with_capacity(source.pixels().len());
    for &[red, green, blue, _] in source.pixels().as_chunks::<RGBA>().0 {
        let luminance = texture_pixels::luminance([red, green, blue].map(decode));
        let colour = luminance * level_scale * COLOUR_PER_LEVEL;
        // A neutral specular colour above 1 saturates in the exported RGB texture.
        let value = texture_pixels::encode(colour);
        pixels.extend_from_slice(&[value, value, value, texture_pixels::OPAQUE]);
    }
    RgbaImage::new(source.width(), source.height(), pixels).map_err(|error| error.to_string())
}
