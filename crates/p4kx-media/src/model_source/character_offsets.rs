// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::character_definition::{
    AttachmentPose, AttachmentPoseField, relative_position, relative_rotation,
};

use super::character_origin::CharacterElement;
use super::character_payload::BoneOffset;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) const POSE_FIELDS: [AttachmentPoseField; 4] = [
    AttachmentPoseField::RelativePosition,
    AttachmentPoseField::RelativeRotation,
    AttachmentPoseField::ModelPosition,
    AttachmentPoseField::ModelRotation,
];

pub(super) struct CharacterOffsets {
    pub(super) relative_position: BoneOffset<[f32; 3]>,
    pub(super) relative_rotation: BoneOffset<[f32; 4]>,
    pub(super) model_position: Option<BoneOffset<[f32; 3]>>,
    pub(super) model_rotation: Option<BoneOffset<[f32; 4]>>,
    pub(super) decoded: AttachmentPose,
}

impl CharacterOffsets {
    pub(super) fn read(
        element: &CharacterElement<'_, '_>,
        is_bone: bool,
        kind: &OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let mut offset = |field: AttachmentPoseField, path, is_optional| {
            let name = field.name();
            if is_optional && !element.has_attribute(name) {
                return Ok(None);
            }
            element.offset(name, path, kind, prepared, writer)
        };
        let [rp, rr, mp, mr] = POSE_FIELDS;
        let relative_pos = offset(rp, "/data/payload/relative_position", false)?;
        let relative_rot = offset(rr, "/data/payload/relative_rotation_wxyz", false)?;
        let model_pos = offset(mp, "/data/payload/model_position", true)?;
        let model_rot = offset(mr, "/data/payload/model_rotation_wxyz", true)?;
        let observed = [
            (rp, relative_pos),
            (rr, relative_rot),
            (mp, model_pos),
            (mr, model_rot),
        ];
        let decoded = AttachmentPose::read(|field| {
            observed
                .iter()
                .find(|(known, _)| *known == field)
                .and_then(|(_, value)| *value)
        });
        Ok(Self {
            relative_position: read_value(relative_pos, is_bone, relative_position),
            relative_rotation: read_value(relative_rot, is_bone, relative_rotation),
            model_position: model_pos
                .map(|text| read_value(Some(text), is_bone, relative_position)),
            model_rotation: model_rot
                .map(|text| read_value(Some(text), is_bone, relative_rotation)),
            decoded,
        })
    }
}

fn read_value<T>(
    text: Option<&str>,
    is_bone: bool,
    parse: impl FnOnce(Option<&str>) -> Option<T>,
) -> BoneOffset<T> {
    BoneOffset {
        authored: text.map(str::to_owned),
        value: is_bone.then(|| parse(text)).flatten(),
    }
}
