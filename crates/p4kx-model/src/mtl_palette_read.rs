// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_direct_read;
use crate::mtl_material_inputs::MaterialSites;
use crate::mtl_read_site::{FieldElement, FieldToken, ReadSite};
use crate::mtl_role_read::TextureInput;
use crate::mtl_shader::GLASS_SHADER;
use crate::mtl_stencil_read::{self, StencilInput};
use crate::mtl_xml::XmlElement;
use crate::{
    Material, MaterialError, MaterialReadError, MaterialReadField, MaterialReadObservation,
    MaterialReadUse, PaletteSlot, StencilTint,
};

pub(crate) fn slot<'source, Interrupted>(
    element: impl XmlElement<'source>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Option<PaletteSlot>, Interrupted> {
    site.at(
        FieldElement::Material,
        MaterialReadField::Palette,
        "MatLayers",
        MaterialReadUse::Selected,
        observer,
    )?;
    for (parent, layers) in element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("MatLayers"))
    {
        for (ordinal, layer) in layers
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Layer"))
        {
            let input = layer.positioned_attribute("PaletteTint");
            let number = input
                .as_ref()
                .and_then(|input| input.value.trim().parse::<u32>().ok());
            site.token(
                FieldElement::Grandchild { parent, ordinal },
                MaterialReadField::Palette,
                FieldToken::attribute(
                    "PaletteTint",
                    input.as_ref(),
                    mtl_direct_read::numeric_use(number.is_some()),
                ),
                observer,
            )?;
            if let Some(slot) = number.and_then(PaletteSlot::of_layer) {
                return Ok(Some(slot));
            }
        }
    }
    glass(element, site, observer)
}

fn glass<'source, Interrupted>(
    element: impl XmlElement<'source>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Option<PaletteSlot>, Interrupted> {
    let shader = element.positioned_attribute("Shader");
    let glass = shader
        .as_ref()
        .is_some_and(|shader| shader.value.eq_ignore_ascii_case(GLASS_SHADER));
    let mask = element.positioned_attribute("StringGenMask");
    let uses_palette = mask
        .as_ref()
        .is_some_and(|mask| mask.value.to_ascii_uppercase().contains("USE_TINT_PALETTE"));
    site.token(
        FieldElement::Material,
        MaterialReadField::Palette,
        FieldToken::attribute(
            "Shader",
            shader.as_ref(),
            mtl_direct_read::numeric_use(shader.is_some()),
        ),
        observer,
    )?;
    if glass {
        site.token(
            FieldElement::Material,
            MaterialReadField::Palette,
            FieldToken::attribute(
                "StringGenMask",
                mask.as_ref(),
                mtl_direct_read::numeric_use(mask.is_some()),
            ),
            observer,
        )?;
    }
    Ok((glass && uses_palette).then_some(PaletteSlot::Glass))
}

pub(crate) fn stencil<Interrupted>(
    material: &Material,
    sites: &MaterialSites<'_>,
    site: ReadSite,
    shader: FieldToken<'_>,
    mask: FieldToken<'_>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Option<PaletteSlot>, MaterialReadError<Interrupted>> {
    let result = mtl_stencil_read::motif(material, &mut |input| {
        stencil_input(input, sites, site, shader, mask, observer)
    })?;
    Ok(result
        .filter(|stencil| stencil.tint != StencilTint::None)
        .map(|stencil| {
            if stencil.tint == StencilTint::DecalColours {
                PaletteSlot::Decal
            } else {
                PaletteSlot::Primary
            }
        }))
}

fn stencil_input<Interrupted>(
    input: StencilInput,
    sites: &MaterialSites<'_>,
    site: ReadSite,
    shader: FieldToken<'_>,
    mask: FieldToken<'_>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    let (element, token) = match input {
        StencilInput::Shader => (FieldElement::Material, interpreted(shader)),
        StencilInput::StringGenMask => (FieldElement::Material, interpreted(mask)),
        StencilInput::Textures => (
            FieldElement::Material,
            FieldToken::attribute("Textures", None, MaterialReadUse::Selected),
        ),
        StencilInput::Texture { index, field } => {
            let texture = sites.textures.get(index).ok_or(MaterialError::SourceSpan)?;
            let token = match field {
                TextureInput::Map => FieldToken {
                    name: "Map",
                    ordinal: texture.map_attribute,
                    use_kind: texture.map_use,
                },
                TextureInput::File => FieldToken {
                    name: "File",
                    ordinal: Some(texture.file_attribute),
                    use_kind: MaterialReadUse::Interpreted,
                },
            };
            (
                FieldElement::Grandchild {
                    parent: texture.list,
                    ordinal: texture.ordinal,
                },
                token,
            )
        }
        StencilInput::Tint { index, valid } => {
            return stencil_tint(index, valid, sites, site, observer);
        }
    };
    site.token(element, MaterialReadField::Palette, token, observer)
        .map_err(MaterialReadError::Interrupted)
}

fn stencil_tint<Interrupted>(
    index: Option<usize>,
    valid: bool,
    sites: &MaterialSites<'_>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    site.at(
        FieldElement::Material,
        MaterialReadField::Palette,
        "PublicParams",
        MaterialReadUse::Selected,
        observer,
    )
    .map_err(MaterialReadError::Interrupted)?;
    let (element, token) = if let Some(index) = index {
        let tint = sites
            .tint
            .filter(|tint| tint.index == index)
            .ok_or(MaterialError::SourceSpan)?;
        (
            FieldElement::Child {
                ordinal: tint.ordinal,
            },
            FieldToken {
                name: tint.field,
                ordinal: Some(tint.attribute),
                use_kind: mtl_direct_read::numeric_use(valid),
            },
        )
    } else {
        (
            FieldElement::Material,
            FieldToken::attribute("StencilTintOverride", None, MaterialReadUse::Selected),
        )
    };
    site.token(element, MaterialReadField::Palette, token, observer)
        .map_err(MaterialReadError::Interrupted)
}

const fn interpreted(mut original: FieldToken<'_>) -> FieldToken<'_> {
    if matches!(original.use_kind, MaterialReadUse::Authored) {
        original.use_kind = MaterialReadUse::Interpreted;
    }
    original
}
