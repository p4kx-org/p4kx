// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Animation, Bone};
use serde_json::{Value, json};

use super::super::super::facial::FaceLink;
use super::{ACTION_NAMES, FaceRotationComparison};

#[derive(Debug)]
pub(super) struct ClipFrames {
    name: String,
    path: String,
    frame_rate: u16,
    declared_end: u32,
    last_key: u16,
    end: u32,
}

impl ClipFrames {
    pub(super) fn read(animation: &Animation) -> Result<Self, String> {
        if animation.frames_per_second == 0 {
            return Err("comparison clip frame rate is unavailable".to_owned());
        }
        let last_key = animation
            .tracks
            .iter()
            .flat_map(|track| {
                [
                    track
                        .rotations
                        .as_ref()
                        .and_then(|keys| keys.frames.last().copied()),
                    track
                        .translations
                        .as_ref()
                        .and_then(|keys| keys.frames.last().copied()),
                ]
            })
            .flatten()
            .max()
            .ok_or("comparison clip has no authored key bounds")?;
        Ok(Self {
            name: animation.name.clone(),
            path: animation.path.clone(),
            frame_rate: animation.frames_per_second,
            declared_end: animation.end_frame,
            last_key,
            end: animation.end_frame.max(u32::from(last_key)),
        })
    }

    pub(super) fn check(&self, frames: &[u16]) -> Result<(), String> {
        if let Some(&last) = frames.last()
            && u32::from(last) > self.end
        {
            return Err(format!(
                "comparison frame {last} is outside 0..={}",
                self.end
            ));
        }
        Ok(())
    }

    fn record(&self) -> Value {
        json!({
            "name": self.name,
            "path": self.path,
            "framesPerSecond": self.frame_rate,
            "declaredEndFrame": self.declared_end,
            "lastKeyFrame": self.last_key,
            "firstFrame": 0,
            "endFrame": self.end,
        })
    }
}

#[derive(Debug)]
struct BoneBinding {
    index: usize,
    name: String,
}

#[derive(Debug)]
pub(super) struct JointMapping {
    controller: u32,
    bone: Option<BoneBinding>,
}

impl JointMapping {
    fn record(&self, ordinal: usize) -> Value {
        let bone = self
            .bone
            .as_ref()
            .map(|bone| json!({"index": bone.index, "name": bone.name}));
        json!({"rigJoint": ordinal, "controllerId": self.controller, "bone": bone})
    }
}

pub(super) fn joint_mapping(
    bones: &[Bone],
    link: &FaceLink,
    controllers: &[u32],
) -> Vec<JointMapping> {
    controllers
        .iter()
        .enumerate()
        .map(|(ordinal, &controller)| JointMapping {
            controller,
            bone: link.joints.get(ordinal).and_then(|&index| {
                bones.get(index).map(|bone| BoneBinding {
                    index,
                    name: bone.name.clone(),
                })
            }),
        })
        .collect()
}

impl FaceRotationComparison {
    /// Returns a private verification header followed by one original angle record per frame.
    ///
    /// The order in the header's joints array is the order of each frame's Euler radians.
    /// A missing skeleton binding is null.
    pub fn verification_records(&self) -> impl Iterator<Item = Value> + '_ {
        let header = std::iter::once_with(move || {
            let joints: Vec<_> = self
                .joints
                .iter()
                .enumerate()
                .map(|(ordinal, joint)| joint.record(ordinal))
                .collect();
            json!({
                "record": "comparison",
                "clip": self.clip.record(),
                "joints": joints,
                "candidateActions": ACTION_NAMES,
                "selectedConvention": null,
                "referenceComparison": "notPerformed",
            })
        });
        header.chain(
            self.frames
                .iter()
                .zip(&self.angles_radians)
                .map(move |(&frame, angles)| {
                    json!({
                        "record": "frame",
                        "frame": frame,
                        "seconds": f32::from(frame) / f32::from(self.clip.frame_rate),
                        "eulerRadians": angles,
                    })
                }),
        )
    }
}
