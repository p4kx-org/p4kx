// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::Animation;

use super::super::super::ArmourPart;
use super::super::super::animate_rig::{Loaded, load};
use super::super::super::facial::FaceLink;
use super::report::ClipFrames;
use super::{Armour, ModelProvider};

pub(super) struct Prepared<'a> {
    pub(super) clip: ClipFrames,
    pub(super) part: usize,
    pub(super) selected: &'a ArmourPart,
    pub(super) link: &'a FaceLink,
    pub(super) rig: Loaded,
}

impl<'a> Prepared<'a> {
    pub(super) fn read(
        armour: &'a Armour,
        animation: &Animation,
        models: &dyn ModelProvider,
        frames: &[u16],
    ) -> Result<Self, String> {
        if frames.is_empty() || !frames.is_sorted_by(|first, second| first < second) {
            return Err("comparison frames must be strictly increasing".to_owned());
        }
        let clip = ClipFrames::read(animation)?;
        clip.check(frames)?;
        if animation.additive || !animation.bind(&armour.skeleton.bones).bound.is_empty() {
            return Err("comparison requires an absolute facial-only clip".to_owned());
        }
        let (part, selected, link) = armour.face_part().ok_or("character has no facial rig")?;
        let rig = load(link, models)?;
        Ok(Self {
            clip,
            part,
            selected,
            link,
            rig,
        })
    }
}
