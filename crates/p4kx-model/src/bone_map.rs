// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Reverse;

use crate::streams::element;
use crate::{CorruptChunk, Mesh};

/// The most bones one vertex is weighted to.
pub const MAX_INFLUENCES: usize = 8;

const NARROW_COUNT: usize = 4;
const NARROW_STRIDE: usize = 12;
const WIDE_STRIDE: usize = 24;

/// The bones one vertex is weighted to.
///
/// The entries are ordered by weight, strongest first, and a bone occurs once; the unused entries
/// at the end have weight 0 and joint 0. The weights are as the file stores them (they add up to
/// 255 in every mesh of the real Archives) and are not normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Influences {
    /// The indices of the bones in the skeleton of the level, see [`crate::Model::bones_of`].
    pub joints: [u16; MAX_INFLUENCES],
    /// The weights, 0 to 255.
    pub weights: [u8; MAX_INFLUENCES],
}

impl Influences {
    /// Returns the influences of the (bone, weight) pairs `pairs`: zero weights dropped, a bone
    /// listed twice added up, the rest ordered by weight, the first of equal weights first. Pairs
    /// beyond [`MAX_INFLUENCES`] distinct bones are dropped.
    #[must_use]
    pub fn from_pairs(pairs: impl Iterator<Item = (u16, u8)>) -> Self {
        let mut merged = [(0_u16, 0_u8); MAX_INFLUENCES];
        let mut len = 0;
        for (joint, weight) in pairs.filter(|&(_, weight)| weight > 0) {
            if let Some(slot) = merged.iter_mut().take(len).find(|slot| slot.0 == joint) {
                slot.1 = slot.1.saturating_add(weight);
            } else if let Some(slot) = merged.get_mut(len) {
                *slot = (joint, weight);
                len += 1;
            }
        }
        if let Some(used) = merged.get_mut(..len) {
            used.sort_by_key(|&(_, weight)| Reverse(weight));
        }
        Self {
            joints: merged.map(|(joint, _)| joint),
            weights: merged.map(|(_, weight)| weight),
        }
    }

    /// Returns the influences with every bone `joint` replaced by `map(joint)`; bones that map to
    /// the same bone are added up, and the order is by weight again.
    #[must_use]
    pub fn remapped(&self, map: impl Fn(u16) -> u16) -> Self {
        Self::from_pairs(
            self.joints
                .iter()
                .zip(&self.weights)
                .filter(|&(_, &weight)| weight > 0)
                .map(|(&joint, &weight)| (map(joint), weight)),
        )
    }

    /// Returns the number of bones with a weight above 0.
    #[must_use]
    pub fn count(&self) -> usize {
        self.weights.iter().filter(|&&weight| weight > 0).count()
    }

    /// Returns the sum of the weights.
    #[must_use]
    pub fn sum(&self) -> u32 {
        self.weights.iter().map(|&weight| u32::from(weight)).sum()
    }
}

/// Checks that every bone a vertex of `meshes` has a weight for is one of `bones` bones.
pub(crate) fn check_joints(meshes: &[Mesh], bones: usize) -> Result<(), CorruptChunk> {
    for influences in meshes.iter().filter_map(|mesh| mesh.influences.as_ref()) {
        for (vertex, influence) in influences.iter().enumerate() {
            let weighted = influence.joints.iter().zip(&influence.weights);
            if let Some((&joint, _)) = weighted
                .filter(|&(_, &weight)| weight > 0)
                .find(|&(&joint, _)| usize::from(joint) >= bones)
            {
                return Err(CorruptChunk::JointRange {
                    vertex,
                    joint,
                    bones,
                });
            }
        }
    }
    Ok(())
}

/// The per-vertex bone weights of a mesh: four 16-bit bone indices and four 8-bit weights per
/// vertex, or eight of each for the heads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoneMapStream<'a> {
    data: &'a [u8],
    wide: bool,
}

impl<'a> BoneMapStream<'a> {
    pub(crate) const fn new(data: &'a [u8], wide: bool) -> Self {
        Self { data, wide }
    }

    /// Returns whether the stream carries eight influences per vertex.
    pub(crate) const fn is_wide(&self) -> bool {
        self.wide
    }

    /// Returns the influences of vertex `index`.
    pub(crate) fn influences(&self, index: usize) -> Result<Influences, CorruptChunk> {
        let (stride, joints) = if self.wide {
            (WIDE_STRIDE, MAX_INFLUENCES * 2)
        } else {
            (NARROW_STRIDE, NARROW_COUNT * 2)
        };
        let bytes = element(self.data, index, stride, "bone map")?;
        let (joints, weights) = bytes
            .split_at_checked(joints)
            .ok_or(CorruptChunk::MissingStream("bone map"))?;
        let joints = joints
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair));
        Ok(Influences::from_pairs(joints.zip(weights.iter().copied())))
    }
}

#[cfg(test)]
mod tests {
    use super::Influences;

    #[test]
    fn remapping_adds_up_bones_that_meet() {
        let influences = Influences::from_pairs([(3, 100), (4, 100), (5, 55)].into_iter());
        let remapped = influences.remapped(|joint| if joint < 5 { 1 } else { 2 });
        assert_eq!(remapped.joints.get(..2), Some([1, 2].as_slice()));
        assert_eq!(remapped.weights.get(..2), Some([200, 55].as_slice()));
        assert_eq!(remapped.count(), 2);
    }
}
