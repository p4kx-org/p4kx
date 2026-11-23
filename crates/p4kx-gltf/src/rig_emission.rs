// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::instance::{Instance, MaterialIds};
use crate::layout::Layout;
use crate::materials::Slot;
use crate::rig_addresses::{Addresses, RiggedBufferView, RiggedJsonAddress};
use crate::shading::{shaded_layers, uses_plain_diffuse};
use crate::texture_origins::{TextureMaterialSource, TextureReadUse};
use crate::texture_plan::{SlotImages, TextureSet};

/// The geometry and material slots one supplied part wrote into a rigged GLB.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedPartEmission {
    geometry: bool,
    skinned: bool,
    morphs: bool,
    material_slots: BTreeMap<usize, BTreeSet<usize>>,
    diffuse_slots: BTreeSet<usize>,
    default_layers: Option<BTreeSet<usize>>,
}

impl RiggedPartEmission {
    /// Returns whether the part supplied mesh geometry.
    #[must_use]
    pub const fn geometry(&self) -> bool {
        self.geometry
    }

    /// Returns whether the geometry supplied bone weights.
    #[must_use]
    pub const fn skinned(&self) -> bool {
        self.skinned
    }

    /// Returns whether the geometry supplied morph targets.
    #[must_use]
    pub const fn morphs(&self) -> bool {
        self.morphs
    }

    /// Iterates the library positions used by emitted primitives.
    pub fn material_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.material_slots.keys().copied()
    }

    /// Iterates the authored layer positions used by an emitted library slot.
    pub fn material_layers(&self, slot: usize) -> impl Iterator<Item = usize> + '_ {
        self.material_slots
            .get(&slot)
            .into_iter()
            .flatten()
            .copied()
    }

    /// Returns whether the material factor used this slot's own diffuse colour.
    #[must_use]
    pub fn material_uses_diffuse(&self, slot: usize) -> bool {
        self.diffuse_slots.contains(&slot)
    }

    /// Returns the contributing layer positions of a physically sourced fallback when emitted.
    #[must_use]
    pub const fn archive_default_layers(&self) -> Option<&BTreeSet<usize>> {
        self.default_layers.as_ref()
    }

    fn observe_slot(
        &mut self,
        slot: Slot,
        instance: &Instance<'_>,
        material: &RiggedMaterialEmission,
    ) {
        match slot {
            Slot::Library(position) => {
                self.material_slots
                    .entry(position)
                    .or_default()
                    .extend(material.layers());
                if material.uses_diffuse {
                    self.diffuse_slots.insert(position);
                }
            }
            Slot::Default if instance.plan.has_physical_fallback() => {
                self.default_layers
                    .get_or_insert_default()
                    .extend(material.layers());
            }
            Slot::Default => {}
        }
    }
}

/// A texture binding in an emitted glTF material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RiggedMaterialTexture {
    /// The base colour map.
    BaseColour,
    /// The tangent-space normal map.
    Normal,
    /// The metallic-roughness map.
    MetallicRoughness,
}

impl RiggedMaterialTexture {
    /// Returns the relative glTF JSON pointer of the binding's texture index.
    #[must_use]
    pub const fn index_pointer(self) -> &'static str {
        match self {
            Self::BaseColour => "/pbrMetallicRoughness/baseColorTexture/index",
            Self::Normal => "/normalTexture/index",
            Self::MetallicRoughness => "/pbrMetallicRoughness/metallicRoughnessTexture/index",
        }
    }
}

/// The inputs of one emitted GLB material, identified by its index in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiggedMaterialEmission {
    layers: BTreeSet<usize>,
    uses_diffuse: bool,
    images: [Option<usize>; 3],
}

impl RiggedMaterialEmission {
    /// Iterates the authored layers that affected this material.
    pub fn layers(&self) -> impl Iterator<Item = usize> + '_ {
        self.layers.iter().copied()
    }

    /// Returns whether this material used its authored diffuse colour.
    #[must_use]
    pub const fn uses_diffuse(&self) -> bool {
        self.uses_diffuse
    }

    /// Iterates this material's texture bindings and the embedded image each binding selects.
    pub fn texture_bindings(&self) -> impl Iterator<Item = (RiggedMaterialTexture, usize)> {
        use RiggedMaterialTexture::{BaseColour, MetallicRoughness, Normal};

        [BaseColour, Normal, MetallicRoughness]
            .into_iter()
            .zip(self.images)
            .filter_map(|(binding, image)| image.map(|image| (binding, image)))
    }
}

fn slot_origins(
    part: usize,
    slot: Slot,
    instance: &Instance<'_>,
    images: SlotImages,
    textures: &TextureSet,
) -> RiggedMaterialEmission {
    let material = instance.plan.material(slot);
    let tinted = images.base.is_some_and(|base| base.tinted);
    let layered = images.base.is_some_and(|base| base.layered);
    let roughness = images
        .metallic_roughness
        .is_some_and(|image| image.roughness);
    let metallic = images
        .metallic_roughness
        .is_some_and(|image| image.metallic);
    let mut layers = BTreeSet::new();
    if !tinted || !roughness || (layered && !metallic) {
        layers.extend(
            shaded_layers(material)
                .into_iter()
                .flatten()
                .filter(|layer| layer.share > 0.0)
                .map(|layer| layer.at),
        );
    }
    let uses_diffuse = !tinted && layers.is_empty() && uses_plain_diffuse(material);
    let position = match slot {
        Slot::Library(position) => Some(position),
        Slot::Default => None,
    };
    for image in [images.base.map(|image| image.image), images.normal]
        .into_iter()
        .flatten()
    {
        if let Some(Some(source)) = textures.image_materials.get(image)
            && source.part == part
            && source.slot == position
        {
            layers.extend(&source.layers);
        }
    }
    RiggedMaterialEmission {
        layers,
        uses_diffuse,
        images: [
            images.base.map(|image| image.image),
            images.normal,
            images.metallic_roughness.map(|image| image.image),
        ],
    }
}

/// Evidence identifying the actual emitted geometry of each supplied part, in input order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedGlbEmission {
    parts: Vec<RiggedPartEmission>,
    materials: BTreeMap<usize, RiggedMaterialEmission>,
    images: Vec<Vec<TextureReadUse>>,
    image_materials: Vec<Option<TextureMaterialSource>>,
    addresses: Addresses,
}

impl RiggedGlbEmission {
    pub(crate) fn of(
        instances: &[Instance<'_>],
        layout: &Layout<'_>,
        textures: &mut TextureSet,
        materials: &[MaterialIds],
        addresses: Addresses,
    ) -> Self {
        let mut parts = vec![RiggedPartEmission::default(); instances.len()];
        for mesh in &layout.meshes {
            if let Some(part) = parts.get_mut(mesh.part) {
                part.geometry = true;
                part.skinned |= !mesh.joints.is_empty();
                part.morphs |= !mesh.morphs.is_empty();
            }
        }
        let mut material_emissions = BTreeMap::new();
        for (position, ((part, instance), material_ids)) in
            parts.iter_mut().zip(instances).zip(materials).enumerate()
        {
            let mut observed = BTreeSet::new();
            for (usage, index) in material_ids {
                let Some(slot) = instance.plan.slot(usage.id) else {
                    continue;
                };
                if observed.insert((slot, usage.has_uvs)) {
                    let images = if usage.has_uvs {
                        textures.of(position, slot)
                    } else {
                        SlotImages::default()
                    };
                    let emitted = slot_origins(position, slot, instance, images, textures);
                    part.observe_slot(slot, instance, &emitted);
                    material_emissions.entry(index.value()).or_insert(emitted);
                }
            }
        }
        Self {
            parts,
            materials: material_emissions,
            images: std::mem::take(&mut textures.image_sources),
            image_materials: std::mem::take(&mut textures.image_materials),
            addresses,
        }
    }

    /// Returns the contributions to the material emitted at `index`.
    #[must_use]
    pub fn material(&self, index: usize) -> Option<&RiggedMaterialEmission> {
        self.materials.get(&index)
    }

    /// Iterates the actual sources of each embedded image in its output order.
    pub fn image_sources(&self) -> impl ExactSizeIterator<Item = &[TextureReadUse]> + '_ {
        self.images.iter().map(Vec::as_slice)
    }

    /// Returns the material that supplied the selected image conversion.
    #[must_use]
    pub fn image_material(&self, image: usize) -> Option<&TextureMaterialSource> {
        self.image_materials.get(image)?.as_ref()
    }

    /// Returns the exact binary buffer views in their GLB document order.
    #[must_use]
    pub fn buffer_views(&self) -> &[RiggedBufferView] {
        &self.addresses.views
    }

    /// Returns the authored GLB JSON values whose addresses the writer observed.
    #[must_use]
    pub fn json_addresses(&self) -> &[RiggedJsonAddress] {
        &self.addresses.json
    }

    /// Returns the emission result for each supplied part, including those without geometry.
    #[must_use]
    pub fn parts(&self) -> &[RiggedPartEmission] {
        &self.parts
    }
}
