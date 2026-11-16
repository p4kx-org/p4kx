// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

#[path = "face_rotation_report.rs"]
mod report;

use p4kx_character::FacePose;
use p4kx_gltf::{SampledAnimation, SampledWeights};
use p4kx_model::{Animation, multiply_quaternions};

use super::super::animate_rig::{Loaded, load};
use super::super::facial::FaceLink;
use super::super::sample::{
    evaluate_frames, is_finite, joint_samples_with, turned, weight_samples,
};
use super::{Armour, CharacterAnimations, ModelProvider, face_tracks};

use self::report::ClipFrames;

const ACTION_NAMES: [&str; 4] = [
    "xyz-bind-times-delta",
    "xyz-delta-times-bind",
    "zyx-bind-times-delta",
    "zyx-delta-times-bind",
];

/// Four convention actions and their common original Euler angles.
#[derive(Debug)]
pub struct FaceRotationComparison {
    animations: CharacterAnimations<'static>,
    angles_radians: Vec<Vec<[f32; 3]>>,
    clip: ClipFrames,
    frames: Vec<u16>,
    joints: Vec<report::JointMapping>,
}

impl FaceRotationComparison {
    /// Returns the four actions over the shared rig and frame times.
    #[must_use]
    pub const fn animations(&self) -> &CharacterAnimations<'static> {
        &self.animations
    }

    /// Returns the original Euler angles in frame order, then rig-joint order.
    #[must_use]
    pub const fn angles_radians(&self) -> &[Vec<[f32; 3]>] {
        self.angles_radians.as_slice()
    }
}

impl Armour {
    /// Fits the four facial rotation conventions at exact requested frames for verification.
    ///
    /// # Errors
    /// Returns an error for unavailable source bounds, invalid frames or an unavailable rig.
    /// The clip must be absolute and bind to facial controls only.
    pub fn verify_face_rotations(
        &self,
        animation: &Animation,
        models: &dyn ModelProvider,
        frames: &[u16],
    ) -> Result<FaceRotationComparison, String> {
        if frames.is_empty() || !frames.is_sorted_by(|first, second| first < second) {
            return Err("comparison frames must be strictly increasing".to_owned());
        }
        let clip = ClipFrames::read(animation)?;
        clip.check(frames)?;
        if animation.additive || !animation.bind(&self.skeleton.bones).bound.is_empty() {
            return Err("comparison requires an absolute facial-only clip".to_owned());
        }
        let (part, _, link) = self.face_part().ok_or("character has no facial rig")?;
        let rig = load(link, models)?;
        let face = &rig.face;
        let logic = face.logic();
        let tracks = face_tracks(animation, logic, &HashSet::new());
        if tracks.controls.is_empty() || !tracks.skipped.is_empty() {
            return Err("comparison needs a completely bound facial control sequence".to_owned());
        }
        let observed = evaluate_frames(logic.controls(), &tracks.controls, frames, |values| {
            let mut angles = Vec::with_capacity(face.joints());
            let pose = face.pose_observing_angles(values, |angle| angles.push(angle));
            (pose, angles)
        });
        let (poses, angles_radians): (Vec<_>, Vec<_>) = observed.into_iter().unzip();
        if !poses.iter().all(is_finite) {
            return Err("facial comparison pose is not finite".to_owned());
        }
        let times: Vec<_> = frames
            .iter()
            .map(|&frame| animation.seconds(u32::from(frame)))
            .collect();
        let actions = comparison_actions(self, link, &rig, (part, &times), poses, &angles_radians);
        let joints = report::joint_mapping(&self.skeleton.bones, link, logic.joint_hashes());
        if joints.len() != face.joints() {
            return Err("facial rig joint order does not cover the observed angles".to_owned());
        }
        Ok(FaceRotationComparison {
            animations: comparison_animation_set(actions, rig),
            angles_radians,
            clip,
            frames: frames.to_vec(),
            joints,
        })
    }
}

fn comparison_actions(
    armour: &Armour,
    link: &FaceLink,
    rig: &Loaded,
    (part, times): (usize, &[f32]),
    mut poses: Vec<FacePose>,
    angles_radians: &[Vec<[f32; 3]>],
) -> [SampledAnimation; ACTION_NAMES.len()] {
    let weights: Vec<SampledWeights> = armour
        .weighed(part, link)
        .iter()
        .flat_map(|(at, numbers)| weight_samples(&poses, numbers, *at))
        .collect();
    let action = |name: &str, poses: &[FacePose], rotation: fn([f32; 4], [f32; 4]) -> [f32; 4]| {
        let mut skipped = Vec::new();
        let bones = joint_samples_with(
            &armour.skeleton.bones,
            poses,
            link,
            (rig.face.logic().joint_hashes(), &mut skipped),
            rotation,
        );
        SampledAnimation::new(
            name.to_owned(),
            times.to_vec(),
            bones,
            weights.clone(),
            skipped,
        )
    };

    let [xyz_right, xyz_left, zyx_right, zyx_left] = ACTION_NAMES;
    let xyz_right = action(xyz_right, &poses, turned);
    let xyz_left = action(xyz_left, &poses, left);
    for (pose, angles) in poses.iter_mut().zip(angles_radians) {
        for (rotation, &angles) in pose.rotations.iter_mut().zip(angles) {
            *rotation = reversed_euler(angles);
        }
    }
    [
        xyz_right,
        xyz_left,
        action(zyx_right, &poses, turned),
        action(zyx_left, &poses, left),
    ]
}

fn comparison_animation_set(
    sampled: [SampledAnimation; ACTION_NAMES.len()],
    rig: Loaded,
) -> CharacterAnimations<'static> {
    let values = sampled
        .iter()
        .map(|sample| {
            sample.times.len() * (1 + 7 * sample.bones.len())
                + sample
                    .weights
                    .iter()
                    .map(|weights| weights.values.len())
                    .sum::<usize>()
        })
        .sum();
    CharacterAnimations {
        sampled: Vec::from(sampled),
        sampled_indices: vec![0; ACTION_NAMES.len()],
        sampled_rigs: vec![rig.origin; ACTION_NAMES.len()],
        values,
        ..CharacterAnimations::default()
    }
}

fn left(bind: [f32; 4], delta: [f32; 4]) -> [f32; 4] {
    multiply_quaternions(delta, bind)
}

fn reversed_euler([x, y, z]: [f32; 3]) -> [f32; 4] {
    let (sx, cx) = (x * 0.5).sin_cos();
    let (sy, cy) = (y * 0.5).sin_cos();
    let (sz, cz) = (z * 0.5).sin_cos();
    let qx = [sx, 0.0, 0.0, cx];
    let qy = [0.0, sy, 0.0, cy];
    let qz = [0.0, 0.0, sz, cz];
    multiply_quaternions(multiply_quaternions(qz, qy), qx)
}
