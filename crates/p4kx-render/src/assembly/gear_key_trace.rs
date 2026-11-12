// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Arc;

use p4kx_assembly::{GearAnimationKey, GearPoseInput};

use crate::scene::{Bounds, Vertex};

#[derive(Clone, Copy)]
pub(super) struct GearMeshInput<'a> {
    pub(super) pose: GearPoseInput<'a>,
    pub(super) lod_ordinal: usize,
    pub(super) mesh: usize,
}

impl GearMeshInput<'_> {
    fn keys(self, vertices: impl Iterator<Item = usize>) -> BTreeSet<GearAnimationKey> {
        self.pose
            .keys_for_vertices(self.lod_ordinal, self.mesh, vertices)
    }
}

#[derive(Debug, Default)]
pub(super) struct GearKeyTrace {
    bounds: [BTreeSet<GearAnimationKey>; 6],
    pub(super) ranges: Vec<Option<Arc<[GearAnimationKey]>>>,
}

impl GearKeyTrace {
    pub(super) fn update_bounds(
        &mut self,
        mesh: GearMeshInput<'_>,
        vertices: &[Vertex],
        before: Bounds,
        after: Bounds,
    ) {
        let old = before.min.into_iter().chain(before.max);
        let new = after.min.into_iter().chain(after.max);
        let mut differences = old
            .zip(new)
            .map(|(old, new)| (old.to_bits() != new.to_bits()).then_some(new));
        let changed: [Option<f32>; 6] = std::array::from_fn(|_| differences.next().flatten());
        if changed.iter().all(Option::is_none) {
            return;
        }
        let mut found = [None; 6];
        for (index, vertex) in vertices.iter().enumerate() {
            let [x, y, z, ..] = *vertex;
            let point = [x, y, z];
            if !point.into_iter().all(f32::is_finite) {
                continue;
            }
            for (axis, (slot, target)) in found.iter_mut().zip(changed).enumerate() {
                if let (Some(target), Some(value)) = (target, point.get(axis % 3))
                    && value.to_bits() == target.to_bits()
                {
                    slot.get_or_insert(index);
                }
            }
            if changed
                .iter()
                .zip(&found)
                .all(|(target, index)| target.is_none() || index.is_some())
            {
                break;
            }
        }
        for ((target, index), changed) in self.bounds.iter_mut().zip(found).zip(changed) {
            if changed.is_some() {
                *target = mesh.keys(index.into_iter());
            }
        }
    }

    pub(super) fn range(&mut self, mesh: GearMeshInput<'_>, indices: &[u32]) {
        let keys = mesh.keys(
            indices
                .iter()
                .filter_map(|&index| usize::try_from(index).ok()),
        );
        self.ranges
            .push((!keys.is_empty()).then(|| keys.into_iter().collect()));
    }

    pub(super) fn bound_keys(self) -> Vec<GearAnimationKey> {
        self.bounds
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
