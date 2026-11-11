// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::instance::Instance;
use crate::layout::Layout;
use crate::materials::Slot;
use crate::shading::shaded_layers;
use crate::texture_origins::{TextureMaterialSource, TextureReadUse};
use crate::texture_plan::TextureSet;

/// The geometry and material slots one supplied part wrote into a rigged GLB.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedPartEmission {
    geometry: bool,
    skinned: bool,
    morphs: bool,
    material_slots: BTreeMap<usize, BTreeSet<usize>>,
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

    fn observe_slot(
        &mut self,
        part: usize,
        slot: usize,
        instance: &Instance<'_>,
        textures: &TextureSet,
    ) {
        self.material_slots.entry(slot).or_insert_with(|| {
            let material_slot = Slot::Library(slot);
            let images = textures.of(part, material_slot);
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
                    shaded_layers(instance.plan.material(material_slot))
                        .into_iter()
                        .flatten()
                        .filter(|layer| layer.share > 0.0)
                        .map(|layer| layer.at),
                );
            }
            for image in [images.base.map(|image| image.image), images.normal]
                .into_iter()
                .flatten()
            {
                if let Some(Some(material)) = textures.image_materials.get(image)
                    && material.part == part
                    && material.slot == Some(slot)
                {
                    layers.extend(&material.layers);
                }
            }
            layers
        });
    }
}

/// Evidence identifying the actual emitted geometry of each supplied part, in input order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedGlbEmission {
    parts: Vec<RiggedPartEmission>,
    images: Vec<Vec<TextureReadUse>>,
    image_materials: Vec<Option<TextureMaterialSource>>,
}

impl RiggedGlbEmission {
    pub(crate) fn of(
        instances: &[Instance<'_>],
        layout: &Layout<'_>,
        textures: &mut TextureSet,
    ) -> Self {
        let mut parts = vec![RiggedPartEmission::default(); instances.len()];
        for mesh in &layout.meshes {
            let (Some(part), Some(instance)) = (parts.get_mut(mesh.part), instances.get(mesh.part))
            else {
                continue;
            };
            part.geometry = true;
            part.skinned |= !mesh.joints.is_empty();
            part.morphs |= !mesh.morphs.is_empty();
            for primitive in &mesh.primitives {
                if let Some(Slot::Library(position)) = instance.plan.slot(primitive.material_id) {
                    part.observe_slot(mesh.part, position, instance, textures);
                }
            }
        }
        Self {
            parts,
            images: std::mem::take(&mut textures.image_sources),
            image_materials: std::mem::take(&mut textures.image_materials),
        }
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

    /// Returns the emission result for each supplied part, including those without geometry.
    #[must_use]
    pub fn parts(&self) -> &[RiggedPartEmission] {
        &self.parts
    }
}
