// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! How a material is blended: its alpha mode, factor and cutoff.

use gltf_json as json;
use json::material::AlphaMode;
use p4kx_model::Material;

use crate::materials::{DECAL_SHADERS, is_one_of};
use crate::texture_roles::sources_with;

const GLASS_ALPHA: f32 = 0.3;
const HAIR_CARD_SHADERS: [&str; 1] = ["HairPBR"];
const HAIR_CARD_CUTOFF: f32 = 0.5;

/// The alpha mode of `material`, its alpha factor and its alpha cutoff.
///
/// The opacity of a stencil decal drawn by its stencil map counts times its `StencilOpacity`.
pub(crate) fn alpha_of(material: &Material) -> (AlphaMode, f32, Option<f32>) {
    let stencil = material
        .stencil_motif()
        .map_or(1.0, |stencil| stencil.opacity);
    let opacity = material.opacity.clamp(0.0, 1.0) * stencil;
    let decal = is_one_of(&material.shader, &DECAL_SHADERS);
    let glass = material.is_glass();
    if material.alpha_test > 0.0 {
        (
            AlphaMode::Mask,
            1.0,
            Some(material.alpha_test.clamp(0.0, 1.0)),
        )
    } else if material.is_hair_cap() {
        (AlphaMode::Blend, 1.0, None)
    } else if is_hair_cards(material) {
        (AlphaMode::Mask, 1.0, Some(HAIR_CARD_CUTOFF))
    } else if glass {
        (AlphaMode::Blend, opacity.min(GLASS_ALPHA), None)
    } else if decal || opacity < 1.0 {
        (AlphaMode::Blend, opacity, None)
    } else {
        (AlphaMode::Opaque, 1.0, None)
    }
}

/// Returns whether `material` draws hair cards: a `HairPBR` material without a diffuse map whose
/// `TexSlot1` is an opacity map, which cuts the cards out.
pub(crate) fn is_hair_cards(material: &Material) -> bool {
    is_one_of(&material.shader, &HAIR_CARD_SHADERS)
        && !material.is_hair_cap()
        && sources_with(material, true)
            .diffuse
            .is_some_and(|source| source.opacity)
}

/// Returns whether `material` is drawn with transparency, so that the alpha channel of its
/// diffuse texture counts.
pub(crate) fn uses_alpha(material: &Material) -> bool {
    alpha_of(material).0 != AlphaMode::Opaque
}

/// Returns whether the shape of `material` is the alpha of its base colour image: a hair cap and
/// any stencil decal, drawn by its stencil map or by its diffuse map. Without that image they are
/// fully transparent, never an opaque shell or a solid quad.
pub(crate) fn is_cut_out_by_base_image(material: &Material) -> bool {
    material.is_hair_cap() || material.is_stencil_decal()
}
