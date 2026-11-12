// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_raw::{self, Attributes, Budget};
use crate::mtl_read_site::{FieldElement, FieldToken, ReadSite};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::{PositionedAttribute, XmlElement};
use crate::{
    MaterialError, MaterialLayer, MaterialReadField, MaterialReadObservation, MaterialReadUse,
    TextureSlot,
};

#[derive(Clone, Copy)]
pub(crate) struct TextureSite {
    pub(crate) list: usize,
    pub(crate) ordinal: usize,
    pub(crate) map_use: MaterialReadUse,
    pub(crate) map_attribute: Option<usize>,
    pub(crate) file_attribute: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct TintSite<'source> {
    pub(crate) ordinal: usize,
    pub(crate) field: &'source str,
    pub(crate) index: usize,
    pub(crate) attribute: usize,
}

pub(crate) struct MaterialSites<'source> {
    pub(crate) textures: Vec<TextureSite>,
    pub(crate) tint: Option<TintSite<'source>>,
}

pub(crate) fn textures<'source, Interrupted>(
    element: impl XmlElement<'source>,
    budget: &mut Budget,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(Vec<TextureSlot>, MaterialSites<'source>), MaterialReadError<Interrupted>> {
    collect_site(
        site,
        FieldElement::Material,
        MaterialReadField::Textures,
        "Textures",
        observer,
    )?;
    let mut slots = Vec::new();
    let mut sites = MaterialSites {
        textures: Vec::new(),
        tint: None,
    };
    for (list, textures) in element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("Textures"))
    {
        collect_site(
            site,
            FieldElement::Child { ordinal: list },
            MaterialReadField::Textures,
            "textureFileAdmission",
            observer,
        )?;
        for (ordinal, texture) in textures
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Texture"))
        {
            let Some(file) = texture
                .positioned_attribute("File")
                .filter(|file| !file.value.is_empty())
            else {
                continue;
            };
            let map = texture.positioned_attribute("Map");
            let parsed = slot(
                texture,
                file.value,
                map.as_ref().map(|map| map.value),
                budget,
            )?;
            let input = FieldElement::Grandchild {
                parent: list,
                ordinal,
            };
            let index = slots.len();
            observe_texture(site, input, index, &file, map.as_ref(), observer)?;
            let map_attribute = map.as_ref().map(|map| map.ordinal);
            slots.push(parsed.value);
            sites.textures.push(TextureSite {
                list,
                ordinal,
                map_use: parsed.map_use,
                map_attribute,
                file_attribute: file.ordinal,
            });
        }
    }
    Ok((slots, sites))
}

fn observe_texture<Interrupted>(
    site: ReadSite,
    input: FieldElement,
    index: usize,
    file: &PositionedAttribute<'_>,
    map: Option<&PositionedAttribute<'_>>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    collect_site(
        site,
        input,
        MaterialReadField::Texture { index },
        "Texture",
        observer,
    )?;
    site.token(
        input,
        MaterialReadField::Texture { index },
        FieldToken {
            name: file.name,
            ordinal: Some(file.ordinal),
            use_kind: MaterialReadUse::Selected,
        },
        observer,
    )
    .map_err(MaterialReadError::Interrupted)?;
    token_site(
        site,
        input,
        MaterialReadField::TextureFile { index },
        file.name,
        file.ordinal,
        observer,
    )?;
    if let Some(map) = map {
        token_site(
            site,
            input,
            MaterialReadField::TextureMap { index },
            map.name,
            map.ordinal,
            observer,
        )?;
    } else {
        collect_site(
            site,
            input,
            MaterialReadField::TextureMap { index },
            "Map",
            observer,
        )?;
    }
    Ok(())
}

struct ParsedTexture {
    value: TextureSlot,
    map_use: MaterialReadUse,
}

fn slot<'source>(
    element: impl XmlElement<'source>,
    file: &str,
    map: Option<&str>,
    budget: &mut Budget,
) -> Result<ParsedTexture, MaterialError> {
    let map_use = if map.is_some() {
        MaterialReadUse::Interpreted
    } else {
        MaterialReadUse::Selected
    };
    Ok(ParsedTexture {
        map_use,
        value: TextureSlot {
            map: map.unwrap_or_default().to_owned(),
            file: file.to_owned(),
            attributes: mtl_raw::attributes(element)
                .into_iter()
                .filter(|(name, _)| {
                    !name.eq_ignore_ascii_case("Map") && !name.eq_ignore_ascii_case("File")
                })
                .collect(),
            children: {
                let mut children = Vec::new();
                for (ordinal, child) in element.children().enumerate() {
                    if let Some(child) = mtl_raw::raw(child, 1, budget, Some(ordinal))? {
                        children.push(child);
                    }
                }
                children
            },
        },
    })
}

pub(crate) fn public_params<'source, Interrupted>(
    element: impl XmlElement<'source>,
    sites: &mut MaterialSites<'source>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Attributes, MaterialReadError<Interrupted>> {
    collect_site(
        site,
        FieldElement::Material,
        MaterialReadField::PublicParams,
        "PublicParams",
        observer,
    )?;
    let mut values = Vec::new();
    for (ordinal, params) in element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("PublicParams"))
    {
        let input = FieldElement::Child { ordinal };
        collect_site(
            site,
            input,
            MaterialReadField::PublicParams,
            "attributes",
            observer,
        )?;
        for (attribute, (name, value)) in params.attributes().enumerate() {
            let index = values.len();
            token_site(
                site,
                input,
                MaterialReadField::PublicParam { index },
                name,
                attribute,
                observer,
            )?;
            if sites.tint.is_none() && name.eq_ignore_ascii_case("StencilTintOverride") {
                sites.tint = Some(TintSite {
                    ordinal,
                    field: name,
                    index: values.len(),
                    attribute,
                });
            }
            values.push((name.to_owned(), value.to_owned()));
        }
    }
    Ok(values)
}

pub(crate) fn layers<'source, Interrupted>(
    element: impl XmlElement<'source>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Vec<MaterialLayer>, MaterialReadError<Interrupted>> {
    collect_site(
        site,
        FieldElement::Material,
        MaterialReadField::Layers,
        "MatLayers",
        observer,
    )?;
    let mut layers = Vec::new();
    let lists = element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("MatLayers"));
    for (parent, list) in lists {
        collect_site(
            site,
            FieldElement::Child { ordinal: parent },
            MaterialReadField::Layers,
            "layerElements",
            observer,
        )?;
        for (ordinal, layer) in list
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Layer"))
        {
            let index = layers.len();
            let input = FieldElement::Grandchild { parent, ordinal };
            collect_site(
                site,
                input,
                MaterialReadField::Layer { index },
                "Layer",
                observer,
            )?;
            let layer = MaterialLayer {
                attributes: mtl_raw::attributes(layer),
                surface: None,
                palette_entry: None,
            };
            for (name, field) in [
                ("Name", MaterialReadField::LayerName { index }),
                ("Path", MaterialReadField::LayerPath { index }),
                ("PaletteTint", MaterialReadField::LayerPaletteTint { index }),
            ] {
                if let Some((attribute, name, _)) = layer.positioned_attribute(name) {
                    token_site(site, input, field, name, attribute, observer)?;
                }
            }
            layers.push(layer);
        }
    }
    Ok(layers)
}

fn collect_site<Interrupted>(
    site: ReadSite,
    input: FieldElement,
    field: MaterialReadField,
    name: &str,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    site.at(input, field, name, MaterialReadUse::Selected, observer)
        .map_err(MaterialReadError::Interrupted)
}

fn token_site<Interrupted>(
    site: ReadSite,
    input: FieldElement,
    field: MaterialReadField,
    name: &str,
    ordinal: usize,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    site.token(
        input,
        field,
        FieldToken {
            name,
            ordinal: Some(ordinal),
            use_kind: MaterialReadUse::Authored,
        },
        observer,
    )
    .map_err(MaterialReadError::Interrupted)
}
