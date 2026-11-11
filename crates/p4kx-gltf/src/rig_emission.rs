// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::instance::Instance;
use crate::layout::Layout;
use crate::materials::Slot;
use crate::texture_origins::TextureReadUse;

/// The geometry and material slots one supplied part wrote into a rigged GLB.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedPartEmission {
    geometry: bool,
    skinned: bool,
    morphs: bool,
    material_slots: BTreeSet<usize>,
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
        self.material_slots.iter().copied()
    }
}

/// Evidence identifying the actual emitted geometry of each supplied part, in input order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiggedGlbEmission {
    parts: Vec<RiggedPartEmission>,
    images: Vec<Vec<TextureReadUse>>,
}

impl RiggedGlbEmission {
    pub(crate) fn of(
        instances: &[Instance<'_>],
        layout: &Layout<'_>,
        images: Vec<Vec<TextureReadUse>>,
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
            part.material_slots
                .extend(mesh.primitives.iter().filter_map(|primitive| {
                    match instance.plan.slot(primitive.material_id) {
                        Some(Slot::Library(position)) => Some(position),
                        _ => None,
                    }
                }));
        }
        Self { parts, images }
    }

    /// Iterates the actual sources of each embedded image in its output order.
    pub fn image_sources(&self) -> impl ExactSizeIterator<Item = &[TextureReadUse]> + '_ {
        self.images.iter().map(Vec::as_slice)
    }

    /// Returns the emission result for each supplied part, including those without geometry.
    #[must_use]
    pub fn parts(&self) -> &[RiggedPartEmission] {
        &self.parts
    }
}
