// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_raw::{self, Attributes, Budget};
use crate::mtl_xml::XmlElement;
use crate::{MaterialError, MaterialReadUse, TextureSlot};

#[derive(Clone, Copy)]
pub(crate) struct TextureSite {
    pub(crate) list: usize,
    pub(crate) ordinal: usize,
    pub(crate) map_use: MaterialReadUse,
}

#[derive(Clone, Copy)]
pub(crate) struct TintSite<'source> {
    pub(crate) ordinal: usize,
    pub(crate) field: &'source str,
    pub(crate) index: usize,
}

pub(crate) struct MaterialSites<'source> {
    pub(crate) textures: Vec<TextureSite>,
    pub(crate) tint: Option<TintSite<'source>>,
}

pub(crate) fn textures<'source>(
    element: impl XmlElement<'source>,
    budget: &mut Budget,
) -> Result<(Vec<TextureSlot>, MaterialSites<'source>), MaterialError> {
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
        for (ordinal, texture) in textures
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Texture"))
        {
            let file = texture.attribute("File").unwrap_or_default();
            if file.is_empty() {
                continue;
            }
            let parsed = slot(texture, file, budget)?;
            slots.push(parsed.value);
            sites.textures.push(TextureSite {
                list,
                ordinal,
                map_use: parsed.map_use,
            });
        }
    }
    Ok((slots, sites))
}

struct ParsedTexture {
    value: TextureSlot,
    map_use: MaterialReadUse,
}

fn slot<'source>(
    element: impl XmlElement<'source>,
    file: &str,
    budget: &mut Budget,
) -> Result<ParsedTexture, MaterialError> {
    let map = element.attribute("Map");
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

pub(crate) fn public_params<'source>(
    element: impl XmlElement<'source>,
    sites: &mut MaterialSites<'source>,
) -> Attributes {
    let mut values = Vec::new();
    for (ordinal, params) in element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("PublicParams"))
    {
        for (name, value) in params.attributes() {
            if sites.tint.is_none() && name.eq_ignore_ascii_case("StencilTintOverride") {
                sites.tint = Some(TintSite {
                    ordinal,
                    field: name,
                    index: values.len(),
                });
            }
            values.push((name.to_owned(), value.to_owned()));
        }
    }
    values
}
