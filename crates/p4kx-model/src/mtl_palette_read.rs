// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_direct_read;
use crate::mtl_material_inputs::MaterialSites;
use crate::mtl_read_site::{FieldElement, ReadSite};
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
            let number = layer
                .attribute("PaletteTint")
                .and_then(|value| value.trim().parse::<u32>().ok());
            site.at(
                FieldElement::Grandchild { parent, ordinal },
                MaterialReadField::Palette,
                "PaletteTint",
                mtl_direct_read::numeric_use(number.is_some()),
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
    let shader = element.attribute("Shader");
    let glass = shader.is_some_and(|shader| shader.eq_ignore_ascii_case(GLASS_SHADER));
    let mask = element.attribute("StringGenMask");
    let uses_palette =
        mask.is_some_and(|mask| mask.to_ascii_uppercase().contains("USE_TINT_PALETTE"));
    site.at(
        FieldElement::Material,
        MaterialReadField::Palette,
        "Shader",
        mtl_direct_read::numeric_use(shader.is_some()),
        observer,
    )?;
    if glass {
        site.at(
            FieldElement::Material,
            MaterialReadField::Palette,
            "StringGenMask",
            mtl_direct_read::numeric_use(mask.is_some()),
            observer,
        )?;
    }
    Ok((glass && uses_palette).then_some(PaletteSlot::Glass))
}

pub(crate) fn stencil<Interrupted>(
    material: &Material,
    sites: &MaterialSites<'_>,
    site: ReadSite,
    shader_use: MaterialReadUse,
    mask_use: MaterialReadUse,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Option<PaletteSlot>, MaterialReadError<Interrupted>> {
    let result = mtl_stencil_read::motif(material, &mut |input| {
        stencil_input(input, sites, site, shader_use, mask_use, observer)
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
    shader_use: MaterialReadUse,
    mask_use: MaterialReadUse,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    let (element, field, use_kind) = match input {
        StencilInput::Shader => (FieldElement::Material, "Shader", interpreted(shader_use)),
        StencilInput::StringGenMask => (
            FieldElement::Material,
            "StringGenMask",
            interpreted(mask_use),
        ),
        StencilInput::Textures => (
            FieldElement::Material,
            "Textures",
            MaterialReadUse::Selected,
        ),
        StencilInput::Texture { index, field } => {
            let texture = sites.textures.get(index).ok_or(MaterialError::SourceSpan)?;
            let (field, use_kind) = match field {
                TextureInput::Map => ("Map", texture.map_use),
                TextureInput::File => ("File", MaterialReadUse::Interpreted),
            };
            (
                FieldElement::Grandchild {
                    parent: texture.list,
                    ordinal: texture.ordinal,
                },
                field,
                use_kind,
            )
        }
        StencilInput::Tint { index, valid } => {
            return stencil_tint(index, valid, sites, site, observer);
        }
    };
    site.at(
        element,
        MaterialReadField::Palette,
        field,
        use_kind,
        observer,
    )
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
    let (element, field, use_kind) = if let Some(index) = index {
        let tint = sites
            .tint
            .filter(|tint| tint.index == index)
            .ok_or(MaterialError::SourceSpan)?;
        (
            FieldElement::Child {
                ordinal: tint.ordinal,
            },
            tint.field,
            mtl_direct_read::numeric_use(valid),
        )
    } else {
        (
            FieldElement::Material,
            "StencilTintOverride",
            MaterialReadUse::Selected,
        )
    };
    site.at(
        element,
        MaterialReadField::Palette,
        field,
        use_kind,
        observer,
    )
    .map_err(MaterialReadError::Interrupted)
}

const fn interpreted(original: MaterialReadUse) -> MaterialReadUse {
    match original {
        MaterialReadUse::Authored => MaterialReadUse::Interpreted,
        other => other,
    }
}
