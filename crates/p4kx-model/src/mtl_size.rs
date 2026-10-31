// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem::size_of;

use crate::{Attributes, Material, MaterialLayer, MaterialLibrary, RawElement, TextureSlot};

const fn text(value: &str) -> usize {
    value.len()
}

fn attributes(list: &Attributes) -> usize {
    list.capacity() * size_of::<(String, String)>()
        + list
            .iter()
            .map(|(name, value)| text(name) + text(value))
            .sum::<usize>()
}

fn element(raw: &RawElement) -> usize {
    text(&raw.name)
        + attributes(&raw.attributes)
        + raw.children.capacity() * size_of::<RawElement>()
        + raw.children.iter().map(element).sum::<usize>()
}

fn elements(list: &[RawElement]) -> usize {
    list.iter().map(element).sum()
}

fn slot(slot: &TextureSlot) -> usize {
    text(&slot.map)
        + text(&slot.file)
        + attributes(&slot.attributes)
        + slot.children.capacity() * size_of::<RawElement>()
        + elements(&slot.children)
}

fn layer(layer: &MaterialLayer) -> usize {
    attributes(&layer.attributes)
        + layer.surface.as_ref().map_or(0, |surface| {
            surface.textures.capacity() * size_of::<TextureSlot>()
                + surface.textures.iter().map(slot).sum::<usize>()
        })
}

fn material(material: &Material) -> usize {
    text(&material.name)
        + text(&material.shader)
        + text(&material.surface_type)
        + text(&material.string_gen_mask)
        + material.textures.capacity() * size_of::<TextureSlot>()
        + material.textures.iter().map(slot).sum::<usize>()
        + attributes(&material.other_attributes)
        + attributes(&material.public_params)
        + material.layers.capacity() * size_of::<MaterialLayer>()
        + material.layers.iter().map(layer).sum::<usize>()
        + material.elements.capacity() * size_of::<RawElement>()
        + elements(&material.elements)
        + material.truncated.capacity() * size_of::<String>()
        + material
            .truncated
            .iter()
            .map(|cut| text(cut))
            .sum::<usize>()
}

impl MaterialLibrary {
    /// Returns the bytes the library holds in memory, struct sizes and heap text counted; an
    /// estimate that ignores allocator overhead.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + text(&self.path)
            + self.default_palette.as_deref().map_or(0, text)
            + self.unresolved_layers.capacity() * size_of::<String>()
            + self
                .unresolved_layers
                .iter()
                .map(|path| text(path))
                .sum::<usize>()
            + self.materials.capacity() * size_of::<Material>()
            + self.materials.iter().map(material).sum::<usize>()
    }
}
