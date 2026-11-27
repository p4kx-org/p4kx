// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#![expect(
    clippy::suboptimal_flops,
    reason = "the quaternion and blend formulas are written as the textbook gives them"
)]

use p4kx_model::{
    Animation, Bone, BoneTransform, Influences, Keys, Mesh, Model, PositionedAnimation,
    chained_world_poses, chained_world_poses_observing,
};

use crate::gear_pose_sources::{GearAnimationKey, GearKeyChannel, PosedBone, PosedLod};

/// Why a model cannot be posed by a clip.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PoseFailure {
    NotSkinned,
    NoBonesAnimated,
}

impl std::fmt::Display for PoseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSkinned => f.write_str("the model is not skinned to bones a clip could move"),
            Self::NoBonesAnimated => f.write_str("the clip moves none of the bones of the model"),
        }
    }
}

/// Returns `model` with every skinned mesh moved to the last pose of `animation`.
///
/// Each bone takes the last key of its track, rotation and translation apart, relative to its
/// parent, and keeps its bind value where the track lacks that channel. The vertices follow the
/// bones by their weights. Meshes without influences stay as they are.
pub(crate) fn posed(
    model: &Model,
    animation: &Animation,
    positions: Option<&PositionedAnimation>,
) -> Result<(Model, Vec<PosedLod>), PoseFailure> {
    let mut posed = model.clone();
    let mut origins = Vec::new();
    let mut moved_any = false;
    let mut skinned = false;
    for (ordinal, lod) in posed.lods.iter_mut().enumerate() {
        let bones = lod.bones.as_deref().unwrap_or(&model.bones);
        if lod.meshes.iter().all(|mesh| mesh.influences.is_none()) {
            continue;
        }
        skinned = true;
        let Some(prepared) = skin_matrices(bones, animation, positions) else {
            continue;
        };
        if let Some(bones) = prepared.sources {
            origins.push(PosedLod { ordinal, bones });
        }
        moved_any = true;
        for mesh in &mut lod.meshes {
            skin(mesh, &prepared.matrices);
        }
    }
    if !skinned {
        return Err(PoseFailure::NotSkinned);
    }
    if !moved_any {
        return Err(PoseFailure::NoBonesAnimated);
    }
    Ok((posed, origins))
}

fn last<T: Copy>(keys: Option<&Keys<T>>) -> Option<T> {
    keys.and_then(|keys| keys.values.last().copied())
}

struct PreparedPose {
    matrices: Vec<BoneTransform>,
    sources: Option<Vec<PosedBone>>,
}

fn skin_matrices(
    bones: &[Bone],
    animation: &Animation,
    positions: Option<&PositionedAnimation>,
) -> Option<PreparedPose> {
    let binding = animation.track_binding_indices(bones);
    if binding.iter().all(Option::is_none) {
        return None;
    }
    let mut sources = positions.map(|_| vec![PosedBone::default(); bones.len()]);
    let mut moved: Vec<Bone> = bones.to_vec();
    for (track_index, (track, index)) in animation.tracks.iter().zip(binding).enumerate() {
        let Some((index, bone)) = index.and_then(|index| Some((index, moved.get_mut(index)?)))
        else {
            continue;
        };
        let rotation = track.rotations.as_ref();
        let translation = track.translations.as_ref();
        if let Some(rotation) = last(rotation) {
            bone.relative.rotation = rotation;
        }
        if let Some(translation) = last(translation) {
            bone.relative.translation = translation;
        }
        if let Some((source, controller)) = sources
            .as_mut()
            .and_then(|sources| sources.get_mut(index))
            .zip(positions.and_then(|positions| positions.controller_ordinal(track_index)))
        {
            source.rotation = key(rotation, controller, GearKeyChannel::Rotation);
            source.translation = key(translation, controller, GearKeyChannel::Translation);
        }
    }
    let poses = chained_world_poses_observing(&moved, |index, parent| {
        if let Some(source) = sources.as_mut().and_then(|sources| sources.get_mut(index)) {
            source.parent = parent;
        }
    });
    let binds = chained_world_poses(bones);
    let matrices = poses
        .iter()
        .zip(&binds)
        .map(|(pose, bind)| pose.then(&inverse(bind)))
        .collect();
    Some(PreparedPose { matrices, sources })
}

fn key<T>(
    keys: Option<&Keys<T>>,
    controller: usize,
    channel: GearKeyChannel,
) -> Option<GearAnimationKey> {
    Some(GearAnimationKey {
        controller,
        ordinal: keys?.values.len().checked_sub(1)?,
        channel,
    })
}

fn inverse(transform: &BoneTransform) -> BoneTransform {
    let [x, y, z, w] = transform.rotation;
    let rotation = [-x, -y, -z, w];
    let moved = rotate(rotation, transform.translation);
    BoneTransform {
        rotation,
        translation: [-moved[0], -moved[1], -moved[2]],
    }
}

fn rotate([x, y, z, w]: [f32; 4], [vx, vy, vz]: [f32; 3]) -> [f32; 3] {
    let tx = 2.0 * (y * vz - z * vy);
    let ty = 2.0 * (z * vx - x * vz);
    let tz = 2.0 * (x * vy - y * vx);
    [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]
}

fn skin(mesh: &mut Mesh, matrices: &[BoneTransform]) {
    let Some(influences) = mesh.influences.as_deref() else {
        return;
    };
    if influences.len() != mesh.positions.len() {
        return;
    }
    for (index, influence) in influences.iter().enumerate() {
        let Some(weights) = Weights::of(influence, matrices) else {
            continue;
        };
        if let Some(position) = mesh.positions.get_mut(index) {
            *position = weights.blend(
                |matrix, vector| {
                    let turned = rotate(matrix.rotation, vector);
                    [
                        turned[0] + matrix.translation[0],
                        turned[1] + matrix.translation[1],
                        turned[2] + matrix.translation[2],
                    ]
                },
                *position,
            );
        }
        if let Some(normal) = mesh.normals.as_mut().and_then(|n| n.get_mut(index)) {
            *normal =
                unit(weights.blend(|matrix, vector| rotate(matrix.rotation, vector), *normal));
        }
        if let Some(tangent) = mesh.tangents.as_mut().and_then(|t| t.get_mut(index)) {
            let [x, y, z, w] = *tangent;
            let [x, y, z] =
                unit(weights.blend(|matrix, vector| rotate(matrix.rotation, vector), [x, y, z]));
            *tangent = [x, y, z, w];
        }
    }
}

fn unit([x, y, z]: [f32; 3]) -> [f32; 3] {
    let length = (x * x + y * y + z * z).sqrt();
    if length > f32::EPSILON {
        [x / length, y / length, z / length]
    } else {
        [x, y, z]
    }
}

struct Weights<'a> {
    parts: [(f32, Option<&'a BoneTransform>); p4kx_model::MAX_INFLUENCES],
}

impl<'a> Weights<'a> {
    fn of(influence: &Influences, matrices: &'a [BoneTransform]) -> Option<Self> {
        let total: u32 = influence
            .weights
            .iter()
            .map(|&weight| u32::from(weight))
            .sum();
        if total == 0 {
            return None;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "weights are at most 255 and their sum at most 2040"
        )]
        let total = total as f32;
        let mut parts = [(0.0, None); p4kx_model::MAX_INFLUENCES];
        for (slot, (&joint, &weight)) in parts
            .iter_mut()
            .zip(influence.joints.iter().zip(&influence.weights))
        {
            if weight > 0 {
                *slot = (f32::from(weight) / total, matrices.get(usize::from(joint)));
            }
        }
        Some(Self { parts })
    }

    fn blend(
        &self,
        apply: impl Fn(&BoneTransform, [f32; 3]) -> [f32; 3],
        vector: [f32; 3],
    ) -> [f32; 3] {
        let mut sum = [0.0_f32; 3];
        let mut used = 0.0_f32;
        for &(weight, matrix) in &self.parts {
            if let Some(matrix) = matrix {
                let moved = apply(matrix, vector);
                for (total, value) in sum.iter_mut().zip(moved) {
                    *total += weight * value;
                }
                used += weight;
            }
        }
        if used > f32::EPSILON {
            sum.map(|value| value / used)
        } else {
            vector
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{inverse, rotate};
    use p4kx_model::BoneTransform;

    #[test]
    fn a_transform_followed_by_its_inverse_changes_nothing() {
        let transform = BoneTransform {
            rotation: [0.5, 0.5, 0.5, 0.5],
            translation: [1.0, 2.0, 3.0],
        };
        let round = transform.then(&inverse(&transform));
        let point = rotate(round.rotation, [1.0, -2.0, 0.5]);
        let moved = [
            point[0] + round.translation[0],
            point[1] + round.translation[1],
            point[2] + round.translation[2],
        ];
        assert!(
            moved
                .iter()
                .zip([1.0, -2.0, 0.5])
                .all(|(a, b)| (a - b).abs() < 1e-5),
            "{moved:?}"
        );
    }

    #[test]
    fn a_quarter_turn_about_z_takes_x_to_y() {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let turned = rotate([0.0, 0.0, half, half], [1.0, 0.0, 0.0]);
        assert!(
            (turned[0]).abs() < 1e-6 && (turned[1] - 1.0).abs() < 1e-6,
            "{turned:?}"
        );
    }
}
