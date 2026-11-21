// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_public_value;
use crate::mtl_role_read::{self, TextureInput};
use crate::mtl_stencil::{self, STENCIL_SLOT, Stencil, StencilTint};
use crate::{Material, TextureSlot};

#[derive(Clone, Copy)]
pub(crate) enum StencilInput {
    Shader,
    StringGenMask,
    Textures,
    Texture { index: usize, field: TextureInput },
    Tint { index: Option<usize>, valid: bool },
}

pub(crate) fn decal<Interrupted>(
    material: &Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<bool, Interrupted> {
    observer(StencilInput::Shader)?;
    if !material.shader.eq_ignore_ascii_case("MeshDecal") {
        return Ok(false);
    }
    observer(StencilInput::StringGenMask)?;
    Ok(material
        .string_gen_mask
        .split('%')
        .any(|flag| flag == "STENCIL_MAP"))
}

pub(crate) fn diffuse<Interrupted>(
    material: &Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<bool, Interrupted> {
    observer(StencilInput::Textures)?;
    for (index, texture) in material.textures.iter().enumerate() {
        if mtl_role_read::diffuse(&texture.map, &texture.file, &mut |field| {
            observer(StencilInput::Texture { index, field })
        })? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn slot<'material, Interrupted>(
    material: &'material Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<Option<&'material TextureSlot>, Interrupted> {
    observer(StencilInput::Textures)?;
    for (index, texture) in material.textures.iter().enumerate() {
        let file = texture.file.trim();
        observer(StencilInput::Texture {
            index,
            field: TextureInput::Map,
        })?;
        if texture.map.eq_ignore_ascii_case(STENCIL_SLOT) {
            observer(StencilInput::Texture {
                index,
                field: TextureInput::File,
            })?;
            if !file.is_empty() && !file.starts_with('$') {
                return Ok(Some(texture));
            }
        }
    }
    Ok(None)
}

pub(crate) fn texture<'material, Interrupted>(
    material: &'material Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<Option<&'material TextureSlot>, Interrupted> {
    if !decal(material, observer)? || diffuse(material, observer)? {
        return Ok(None);
    }
    slot(material, observer)
}

pub(crate) fn parameters<Interrupted>(
    material: &Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<Option<Stencil>, Interrupted> {
    if !decal(material, observer)? {
        return Ok(None);
    }
    let colour = material.public_colour("StencilDiffuseColor");
    let opacity = material
        .public_param("StencilOpacity")
        .map_or(1.0, |opacity| opacity[0].clamp(0.0, 1.0));
    let tint = mtl_public_value::read(material, "StencilTintOverride");
    observer(StencilInput::Tint {
        index: tint.index,
        valid: tint.value.is_some(),
    })?;
    Ok(Some(Stencil {
        colour,
        opacity,
        tint: tint
            .components()
            .map_or(StencilTint::None, |tint| mtl_stencil::tint_of(tint[0])),
    }))
}

pub(crate) fn motif<Interrupted>(
    material: &Material,
    observer: &mut impl FnMut(StencilInput) -> Result<(), Interrupted>,
) -> Result<Option<Stencil>, Interrupted> {
    if texture(material, observer)?.is_some() {
        parameters(material, observer)
    } else {
        Ok(None)
    }
}
