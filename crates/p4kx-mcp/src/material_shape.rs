// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Material;

use crate::budget::Budget;
use crate::error::ToolError;
use crate::material_shapes::{LayerShape, MaterialShape, ParamShape, TextureShape};

pub(crate) fn shape(id: usize, material: &Material) -> Result<MaterialShape, ToolError> {
    let mut text = Budget::new();
    let palette = material
        .palette
        .map(|slot| format!("{slot:?}").to_lowercase());
    if let Some(value) = &palette {
        text.admit_text(value.len())?;
    }
    Ok(MaterialShape {
        id,
        name: text.text(&material.name)?,
        shader: text.text(&material.shader)?,
        surface_type: text.text(&material.surface_type)?,
        string_gen_mask: text.text(&material.string_gen_mask)?,
        flags: material.flags.0,
        two_sided: material.flags.two_sided(),
        no_draw: material.flags.no_draw(),
        diffuse: material.diffuse,
        specular: material.specular,
        emissive: material.emissive,
        shininess: material.shininess,
        opacity: material.opacity,
        alpha_test: material.alpha_test,
        glow: material.glow,
        palette,
        textures: textures(material, &mut text)?,
        texture_count: material.textures.len(),
        public_params: parameters(material, &mut text)?,
        public_param_count: material.public_params.len(),
        layers: layers(material, &mut text)?,
        layer_count: material.layers.len(),
    })
}

fn textures(material: &Material, text: &mut Budget) -> Result<Vec<TextureShape>, ToolError> {
    material
        .textures
        .iter()
        .map(|texture| {
            Ok(TextureShape {
                map: text.text(&texture.map)?,
                file: text.text(&texture.file)?,
            })
        })
        .collect()
}

fn parameters(material: &Material, text: &mut Budget) -> Result<Vec<ParamShape>, ToolError> {
    material
        .public_params
        .iter()
        .map(|(name, value)| {
            Ok(ParamShape {
                name: text.text(name)?,
                value: text.text(value)?,
            })
        })
        .collect()
}

fn layers(material: &Material, text: &mut Budget) -> Result<Vec<LayerShape>, ToolError> {
    material
        .layers
        .iter()
        .map(|layer| {
            Ok(LayerShape {
                name: layer
                    .attribute("Name")
                    .map(|value| text.text(value))
                    .transpose()?,
                path: layer
                    .attribute("Path")
                    .map(|value| text.text(value))
                    .transpose()?,
                palette_tint: layer
                    .attribute("PaletteTint")
                    .map(|value| text.text(value))
                    .transpose()?,
            })
        })
        .collect()
}
