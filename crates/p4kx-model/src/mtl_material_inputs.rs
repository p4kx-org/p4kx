// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_raw::{self, Attributes, Budget};
use crate::mtl_read_site::{FieldElement, FieldToken, ReadSite};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::{PositionedAttribute, XmlElement};
use crate::{
    MaterialError, MaterialReadField, MaterialReadObservation, MaterialReadUse, TextureSlot,
};

#[path = "mtl_layer_inputs.rs"]
mod layer;
pub(crate) use layer::layers;

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
        let mut output = ParsedTextures {
            slots: &mut slots,
            sites: &mut sites,
        };
        for (ordinal, texture) in textures
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Texture"))
        {
            output.push(texture, budget, site, (list, ordinal), observer)?;
        }
    }
    Ok((slots, sites))
}

struct ParsedTextures<'output, 'source> {
    slots: &'output mut Vec<TextureSlot>,
    sites: &'output mut MaterialSites<'source>,
}

impl<'source> ParsedTextures<'_, 'source> {
    fn push<Interrupted>(
        &mut self,
        texture: impl XmlElement<'source>,
        budget: &mut Budget,
        site: ReadSite,
        (list, ordinal): (usize, usize),
        observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<(), MaterialReadError<Interrupted>> {
        let Some(file) = texture
            .positioned_attribute("File")
            .filter(|file| !file.value.is_empty())
        else {
            return Ok(());
        };
        let map = texture.positioned_attribute("Map");
        let index = self.slots.len();
        let input = FieldElement::Grandchild {
            parent: list,
            ordinal,
        };
        let attributes = texture_attributes(texture, site, input, index, observer)?;
        let parsed = slot(
            texture,
            file.value,
            map.as_ref().map(|map| map.value),
            attributes,
            budget,
        )?;
        observe_texture(site, input, index, &file, map.as_ref(), observer)?;
        let map_attribute = map.as_ref().map(|map| map.ordinal);
        self.slots.push(parsed.value);
        self.sites.textures.push(TextureSite {
            list,
            ordinal,
            map_use: parsed.map_use,
            map_attribute,
            file_attribute: file.ordinal,
        });
        Ok(())
    }
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
    attributes: Attributes,
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
            attributes,
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

fn texture_attributes<'source, Interrupted>(
    element: impl XmlElement<'source>,
    site: ReadSite,
    input: FieldElement,
    index: usize,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Attributes, MaterialReadError<Interrupted>> {
    let mut attributes = Vec::new();
    for (ordinal, (name, value)) in element.attributes().enumerate() {
        if name.eq_ignore_ascii_case("Map") || name.eq_ignore_ascii_case("File") {
            continue;
        }
        token_site(
            site,
            input,
            MaterialReadField::TextureAttribute {
                index,
                attribute: attributes.len(),
            },
            name,
            ordinal,
            observer,
        )?;
        attributes.push((name.to_owned(), value.to_owned()));
    }
    Ok(attributes)
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
