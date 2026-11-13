// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{AuthoredAttribute, AuthoredAttributeKind, MaterialLibrary, RawElement};

const REQUIRE_FORWARD_RENDERING: u64 = 0x8000;

#[path = "material_emission_tokens.rs"]
mod tokens;

#[cfg(test)]
#[path = "material_emission_tests.rs"]
mod tests;

pub use tokens::{
    EmissionAttribute, EmissionAttributeKind, EmissionNumber, EmissionNumeric, EmissionTextureSlot,
    EmissionValue,
};

/// Authored emission inputs and source-derived routing declarations of one material.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct MaterialEmission<'a> {
    /// Original element-child ordinals from the material library root.
    pub element: Vec<usize>,
    /// Original emission and coverage attributes in source order.
    pub attributes: Vec<EmissionAttribute<'a>>,
    /// Texture declarations explicitly named by emission slot and shader conventions.
    pub slots: Vec<EmissionTextureSlot<'a>>,
    /// Whether a unique valid authored flag bitset explicitly selects forward rendering.
    pub forward_enabled: Option<bool>,
    /// Whether the shader generation mask explicitly requests an emittance map.
    pub emittance_requested: Option<bool>,
    /// Whether the original material syntax projection reached every child.
    pub source_complete: bool,
}

impl MaterialLibrary {
    /// Returns captured original emission controls of one decoded material position.
    #[must_use]
    pub fn emission_source(&self, ordinal: usize) -> Option<MaterialEmission<'_>> {
        let (material, element) = material_element(self, ordinal)?;
        let mut attributes = Vec::new();
        let authored = material.authored.attributes.iter().enumerate();
        for (attribute, value) in authored {
            if let Some(kind) = EmissionAttributeKind::direct(value) {
                attributes.push(EmissionAttribute::new(
                    kind,
                    element.clone(),
                    attribute,
                    value,
                ));
            }
        }
        for parameters in children(material, "PublicParams") {
            let Some(child) = parameters.authored.ordinal else {
                continue;
            };
            for (attribute, value) in parameters.authored.attributes.iter().enumerate() {
                if let Some(kind) = EmissionAttributeKind::parameter(value) {
                    attributes.push(EmissionAttribute::new(
                        kind,
                        child_path(&element, child),
                        attribute,
                        value,
                    ));
                }
            }
        }
        let flags = unique(&attributes, EmissionAttributeKind::Flags);
        let mask = unique(&attributes, EmissionAttributeKind::ShaderMask);
        let shader = unique(&attributes, EmissionAttributeKind::Shader);
        let forward_enabled = flags
            .and_then(|field| field.value.flags())
            .map(|bits| bits & REQUIRE_FORWARD_RENDERING != 0);
        let emittance_requested = mask.map(|mask| {
            mask.value
                .raw
                .value
                .split('%')
                .any(|token| matches!(token, "EMITTANCE_MAP" | "MACRO_EMISSIVE_MAP"))
        });
        let slots = texture_slots(
            material,
            &element,
            shader.map(|shader| shader.value.raw.value.as_str()),
        );
        Some(MaterialEmission {
            element,
            attributes,
            slots,
            forward_enabled,
            emittance_requested,
            source_complete: self.authored.losses.is_empty(),
        })
    }
}

fn unique<'value, 'source>(
    fields: &'value [EmissionAttribute<'source>],
    kind: EmissionAttributeKind,
) -> Option<&'value EmissionAttribute<'source>> {
    let mut matching = fields.iter().filter(|field| field.kind == kind);
    let first = matching.next()?;
    matching.next().is_none().then_some(first)
}

fn material_element(
    library: &MaterialLibrary,
    ordinal: usize,
) -> Option<(&RawElement, Vec<usize>)> {
    let root = library.authored.root.as_ref()?;
    if library.single {
        return (ordinal == 0).then_some((root, Vec::new()));
    }
    let list = children(root, "SubMaterials").next()?;
    let material = list
        .children
        .iter()
        .find(|row| row.authored.ordinal == Some(ordinal))?;
    material
        .name
        .eq_ignore_ascii_case("Material")
        .then_some((material, vec![list.authored.ordinal?, ordinal]))
}

fn children<'a>(element: &'a RawElement, name: &'a str) -> impl Iterator<Item = &'a RawElement> {
    element
        .children
        .iter()
        .filter(move |child| child.name.eq_ignore_ascii_case(name))
}

fn texture_slots<'a>(
    material: &'a RawElement,
    path: &[usize],
    shader: Option<&str>,
) -> Vec<EmissionTextureSlot<'a>> {
    let mut slots = Vec::new();
    for list in children(material, "Textures") {
        let Some(list_ordinal) = list.authored.ordinal else {
            continue;
        };
        let parent = child_path(path, list_ordinal);
        for texture in children(list, "Texture") {
            let Some(slot) = texture.authored.ordinal else {
                continue;
            };
            let Some((map_ordinal, map)) = attribute(texture, "Map") else {
                continue;
            };
            let explicit = map.value.eq_ignore_ascii_case("Emittance")
                || map.value.eq_ignore_ascii_case("Emissive");
            let hard_surface = shader
                .is_some_and(|shader| shader.eq_ignore_ascii_case("HardSurface"))
                && map.value.eq_ignore_ascii_case("TexSlot14");
            if explicit || hard_surface {
                let element = child_path(&parent, slot);
                slots.push(EmissionTextureSlot::new(
                    element,
                    map_ordinal,
                    map,
                    attribute(texture, "File"),
                ));
            }
        }
    }
    slots
}

fn attribute<'a>(element: &'a RawElement, name: &str) -> Option<(usize, &'a AuthoredAttribute)> {
    let mut matches = element
        .authored
        .attributes
        .iter()
        .enumerate()
        .filter(|(_, attr)| {
            matches!(
                attr.kind,
                AuthoredAttributeKind::Attribute { namespace: None }
            ) && attr.name == name
        });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn child_path(base: &[usize], child: usize) -> Vec<usize> {
    let mut path = base.to_vec();
    path.push(child);
    path
}
