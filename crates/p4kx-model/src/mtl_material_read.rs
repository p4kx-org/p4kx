// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_direct_read::{DirectRead, Identity, Lighting};
use crate::mtl_material_inputs;
use crate::mtl_palette_read;
use crate::mtl_raw::{self, Attributes, Budget};
use crate::mtl_read_site::{FieldToken, ReadSite};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::XmlElement;
use crate::{
    Material, MaterialLayer, MaterialReadObservation, MaterialReadUse, PaletteSlot, RawElement,
    TextureSlot,
};

const TYPED_ATTRIBUTES: [&str; 12] = [
    "Name",
    "MtlFlags",
    "Shader",
    "SurfaceType",
    "StringGenMask",
    "Diffuse",
    "Specular",
    "Emissive",
    "Shininess",
    "Opacity",
    "AlphaTest",
    "Glow",
];

struct Inputs {
    identity: Identity,
    lighting: Lighting,
    palette: Option<PaletteSlot>,
    textures: Vec<TextureSlot>,
    layers: Vec<MaterialLayer>,
    public_params: Attributes,
    elements: Vec<RawElement>,
}

pub(crate) fn material<'source, Interrupted>(
    element: impl XmlElement<'source>,
    budget: &mut Budget,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Material, MaterialReadError<Interrupted>> {
    let (textures, mut sites) = mtl_material_inputs::textures(element, budget, site, observer)?;
    let layers = mtl_material_inputs::layers(element, site, observer)?;
    let elements = mtl_raw::other_elements(element, budget)?;
    let mut read = DirectRead {
        element,
        site,
        observer,
        shader: FieldToken::attribute("Shader", None, MaterialReadUse::Selected),
        mask: FieldToken::attribute("StringGenMask", None, MaterialReadUse::Selected),
    };
    let identity = read.identity().map_err(MaterialReadError::Interrupted)?;
    let lighting = read.lighting().map_err(MaterialReadError::Interrupted)?;
    let palette = mtl_palette_read::slot(element, site, read.observer)
        .map_err(MaterialReadError::Interrupted)?;
    let public_params =
        mtl_material_inputs::public_params(element, &mut sites, site, read.observer)?;
    let mut built = build(
        element,
        Inputs {
            identity,
            lighting,
            palette,
            textures,
            layers,
            public_params,
            elements,
        },
        budget,
    );
    if built.palette.is_none() {
        built.palette =
            mtl_palette_read::stencil(&built, &sites, site, read.shader, read.mask, read.observer)?;
    }
    Ok(built)
}

fn build<'source>(element: impl XmlElement<'source>, inputs: Inputs, budget: &Budget) -> Material {
    Material {
        name: inputs.identity.name,
        flags: inputs.identity.flags,
        shader: inputs.identity.shader,
        surface_type: inputs.identity.surface_type,
        string_gen_mask: inputs.identity.string_gen_mask,
        diffuse: inputs.lighting.diffuse,
        specular: inputs.lighting.specular,
        emissive: inputs.lighting.emissive,
        shininess: inputs.lighting.shininess,
        opacity: inputs.lighting.opacity,
        alpha_test: inputs.lighting.alpha_test,
        glow: inputs.lighting.glow,
        textures: inputs.textures,
        palette: inputs.palette,
        tinted: false,
        decal_colours: None,
        other_attributes: other_attributes(element),
        public_params: inputs.public_params,
        layers: inputs.layers,
        elements: inputs.elements,
        truncated: budget.cut(),
        diffuse_map_is_grey: None,
    }
}

fn other_attributes<'source>(element: impl XmlElement<'source>) -> Attributes {
    mtl_raw::attributes(element)
        .into_iter()
        .filter(|(name, _)| {
            !TYPED_ATTRIBUTES
                .iter()
                .any(|typed| typed.eq_ignore_ascii_case(name))
        })
        .collect()
}
