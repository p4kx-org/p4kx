// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::{Bone, BoneTransform};

impl BoneTransform {
    /// Returns the transform that applies `local` first and then `self`: the pose of a child
    /// with the pose `local` relative to a parent whose pose is `self`.
    #[must_use]
    pub fn then(&self, local: &Self) -> Self {
        let moved = rotate(self.rotation, local.translation);
        Self {
            rotation: multiply(self.rotation, local.rotation),
            translation: [
                self.translation[0] + moved[0],
                self.translation[1] + moved[1],
                self.translation[2] + moved[2],
            ],
        }
    }
}

/// Returns the pose of every bone in the model's space, composed from the `relative` poses.
///
/// It agrees with the node transforms whatever the stored `world` poses say. A bone whose parent
/// is missing counts as a root, and so does the first bone met again along a cycle, the one whose
/// parent is already on the walk.
#[must_use]
pub fn chained_world_poses(bones: &[Bone]) -> Vec<BoneTransform> {
    chained_world_poses_observing(bones, |_, _| {})
}

/// Composes the same poses and reports the actual parent used for each bone.
#[must_use]
pub fn chained_world_poses_observing(
    bones: &[Bone],
    mut observe: impl FnMut(usize, Option<usize>),
) -> Vec<BoneTransform> {
    let mut poses: Vec<Option<BoneTransform>> = vec![None; bones.len()];
    let mut walking = vec![false; bones.len()];
    let mut chain = Vec::new();
    for start in 0..bones.len() {
        chain.clear();
        let mut at = Some(start);
        while let Some(index) = at {
            if poses.get(index).is_none_or(Option::is_some) {
                break;
            }
            if walking.get(index).copied().unwrap_or(true) {
                at = None;
                break;
            }
            if let Some(flag) = walking.get_mut(index) {
                *flag = true;
            }
            chain.push(index);
            at = bones.get(index).and_then(|bone| bone.parent);
        }
        for &index in &chain {
            if let Some(flag) = walking.get_mut(index) {
                *flag = false;
            }
        }
        let mut parent_index = at.filter(|index| poses.get(*index).is_some_and(Option::is_some));
        let mut parent = parent_index
            .and_then(|index| poses.get(index).copied().flatten())
            .unwrap_or(BoneTransform::IDENTITY);
        for &index in chain.iter().rev() {
            let Some(bone) = bones.get(index) else {
                continue;
            };
            let pose = parent.then(&bone.relative);
            observe(index, parent_index);
            parent = pose;
            parent_index = Some(index);
            if let Some(slot) = poses.get_mut(index) {
                *slot = Some(pose);
            }
        }
    }
    poses
        .into_iter()
        .zip(bones)
        .map(|(pose, bone)| pose.unwrap_or(bone.relative))
        .collect()
}

/// Returns the product `a * b` of two quaternions `x, y, z, w`: the rotation that turns by `b`
/// first and then by `a`.
#[must_use]
pub fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw.mul_add(bx, ax.mul_add(bw, ay.mul_add(bz, -(az * by)))),
        aw.mul_add(by, (-ax).mul_add(bz, ay.mul_add(bw, az * bx))),
        aw.mul_add(bz, ax.mul_add(by, (-ay).mul_add(bx, az * bw))),
        aw.mul_add(bw, (-ax).mul_add(bx, (-ay).mul_add(by, -(az * bz)))),
    ]
}

fn rotate(rotation: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let [qx, qy, qz, qw] = rotation;
    let [vx, vy, vz] = vector;
    let tx = 2.0 * qy.mul_add(vz, -(qz * vy));
    let ty = 2.0 * qz.mul_add(vx, -(qx * vz));
    let tz = 2.0 * qx.mul_add(vy, -(qy * vx));
    [
        qw.mul_add(tx, vx) + qy.mul_add(tz, -(qz * ty)),
        qw.mul_add(ty, vy) + qz.mul_add(tx, -(qx * tz)),
        qw.mul_add(tz, vz) + qx.mul_add(ty, -(qy * tx)),
    ]
}

/// Returns `base` with the bones of `extra` that it lacks appended, in the order of `extra`.
///
/// Bones are the same when their names are equal ignoring ASCII case. An appended bone keeps its
/// pose relative to its parent, moved by `moves` (indexed like `extra`, missing entries moving
/// nothing), and hangs below its nearest ancestor in `extra` that the result has; a bone with
/// none becomes a root. The bind poses in the model's space of the appended bones follow from
/// their parents', so the skeleton stays consistent whatever `extra`'s own world poses were.
#[must_use]
pub fn extend_skeleton(base: &[Bone], extra: &[Bone], moves: &[[f32; 3]]) -> Vec<Bone> {
    extend_skeleton_placed(base, extra, moves).0
}

/// Returns [`extend_skeleton`]'s bones and, for each bone of `extra`, its index in them: that of
/// the bone of `base` with the same name, else that of the bone appended for it.
#[must_use]
pub fn extend_skeleton_placed(
    base: &[Bone],
    extra: &[Bone],
    moves: &[[f32; 3]],
) -> (Vec<Bone>, Vec<usize>) {
    let mut by_name: HashMap<String, usize> = HashMap::with_capacity(base.len());
    for (index, bone) in base.iter().enumerate() {
        by_name
            .entry(bone.name.to_ascii_lowercase())
            .or_insert(index);
    }
    let mut next = base.len();
    let placed: Vec<usize> = extra
        .iter()
        .map(|bone| {
            by_name
                .get(&bone.name.to_ascii_lowercase())
                .copied()
                .unwrap_or_else(|| {
                    next += 1;
                    next - 1
                })
        })
        .collect();
    let mut bones = base.to_vec();
    for (index, bone) in extra.iter().enumerate() {
        if placed.get(index).is_none_or(|&at| at < base.len()) {
            continue;
        }
        let mut relative = bone.relative;
        for (value, by) in relative
            .translation
            .iter_mut()
            .zip(moves.get(index).copied().unwrap_or_default())
        {
            *value += by;
        }
        bones.push(Bone {
            name: bone.name.clone(),
            parent: nearest_placed(extra, &placed, index),
            controller_id: bone.controller_id,
            node: None,
            relative,
            world: bone.world,
        });
    }
    settle(&mut bones, base.len());
    (bones, placed)
}

fn nearest_placed(extra: &[Bone], placed: &[usize], start: usize) -> Option<usize> {
    let mut at = extra.get(start)?.parent;
    for _ in 0..extra.len() {
        let index = at?;
        if let Some(&target) = placed.get(index) {
            return Some(target);
        }
        at = extra.get(index)?.parent;
    }
    None
}

fn settle(bones: &mut [Bone], fixed: usize) {
    let mut done = vec![false; bones.len()];
    for flag in done.iter_mut().take(fixed) {
        *flag = true;
    }
    for start in fixed..bones.len() {
        let mut chain = Vec::new();
        let mut at = Some(start);
        while let Some(index) = at.filter(|&index| !done.get(index).copied().unwrap_or(true)) {
            chain.push(index);
            at = bones.get(index).and_then(|bone| bone.parent);
        }
        for index in chain.into_iter().rev() {
            let parent_world = bones
                .get(index)
                .and_then(|bone| bone.parent)
                .and_then(|parent| bones.get(parent))
                .map(|parent| parent.world);
            if let Some(bone) = bones.get_mut(index) {
                bone.world = match parent_world {
                    Some(parent) => parent.then(&bone.relative),
                    None => bone.relative,
                };
            }
            if let Some(flag) = done.get_mut(index) {
                *flag = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(name: &str, parent: Option<usize>, translation: [f32; 3]) -> Bone {
        let relative = BoneTransform {
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation,
        };
        Bone {
            name: name.to_owned(),
            parent,
            controller_id: 0,
            node: None,
            relative,
            world: relative,
        }
    }

    #[test]
    fn missing_bones_hang_below_their_nearest_present_ancestor_and_are_settled() {
        let base = vec![
            bone("Root", None, [0.0, 0.0, 1.0]),
            bone("Head", Some(0), [0.0, 0.0, 2.0]),
        ];
        let extra = vec![
            bone("head", None, [9.0, 9.0, 9.0]),
            bone("leaf", Some(2), [0.0, 1.0, 0.0]),
            bone("mid", Some(0), [1.0, 0.0, 0.0]),
        ];

        let bones = extend_skeleton(&base, &extra, &[[0.0; 3], [0.0, 0.5, 0.0]]);

        assert_eq!(bones.len(), 4);
        assert_eq!(bones[2].name, "leaf");
        assert_eq!(bones[2].parent, Some(3));
        assert_eq!(bones[3].name, "mid");
        assert_eq!(bones[3].parent, Some(1));
        assert_eq!(bones[2].relative.translation, [0.0, 1.5, 0.0]);
        assert_eq!(bones[3].world.translation, [1.0, 0.0, 2.0]);
        assert_eq!(bones[2].world.translation, [1.0, 1.5, 2.0]);
    }

    #[test]
    fn each_extra_bone_is_placed_at_its_namesake_or_its_appended_copy() {
        let base = vec![
            bone("Root", None, [0.0; 3]),
            bone("Head", Some(0), [0.0; 3]),
        ];
        let extra = vec![
            bone("head", None, [0.0; 3]),
            bone("leaf", Some(0), [0.0; 3]),
            bone("mid", Some(0), [0.0; 3]),
        ];

        let (bones, placed) = extend_skeleton_placed(&base, &extra, &[]);

        assert_eq!(placed, [1, 2, 3]);
        assert_eq!(bones.len(), 4);
    }

    #[test]
    fn a_child_pose_is_turned_by_its_parent() {
        let quarter = std::f32::consts::FRAC_1_SQRT_2;
        let parent = BoneTransform {
            rotation: [0.0, 0.0, quarter, quarter],
            translation: [1.0, 0.0, 0.0],
        };
        let child = BoneTransform {
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [1.0, 0.0, 0.0],
        };
        let world = parent.then(&child);
        assert!((world.translation[0] - 1.0).abs() < 1e-6);
        assert!((world.translation[1] - 1.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod multiply_tests {
    use super::multiply;

    #[test]
    fn the_left_factor_is_the_rotated_frame_so_a_rotated_bind_pose_turns_the_delta() {
        let quarter = std::f32::consts::FRAC_1_SQRT_2;
        let about_z = [0.0, 0.0, quarter, quarter];
        let about_x = [quarter, 0.0, 0.0, quarter];

        let bind_then_delta = multiply(about_z, about_x);
        let delta_then_bind = multiply(about_x, about_z);

        assert!((bind_then_delta[0] - 0.5).abs() < 1e-6);
        assert!((bind_then_delta[1] - 0.5).abs() < 1e-6);
        assert!((delta_then_bind[1] + 0.5).abs() < 1e-6);
    }
}
