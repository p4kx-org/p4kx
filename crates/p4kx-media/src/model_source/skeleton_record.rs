// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::BoneTransform;
use schemars::JsonSchema;
use serde::Serialize;

pub(super) const CHUNK: &str = "model-skeleton-chunk";
pub(super) const BONE: &str = "model-skeleton-bone";

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SkeletonState {
    Readable,
    UnsupportedVersion,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SkeletonChunk {
    pub(super) disposition: SkeletonState,
    pub(super) bone_count: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Pose {
    rotation_xyzw: [f32; 4],
    translation_z_up_metres: [f32; 3],
}

impl From<BoneTransform> for Pose {
    fn from(pose: BoneTransform) -> Self {
        Self {
            rotation_xyzw: pose.rotation,
            translation_z_up_metres: pose.translation,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SkeletonBone<'a> {
    pub(super) name: &'a str,
    pub(super) name_lossy: bool,
    pub(super) parent: Option<usize>,
    pub(super) controller_id: u32,
    pub(super) node: Option<usize>,
    pub(super) relative: Pose,
    pub(super) model_space: Pose,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum SkeletonRecord<'a> {
    SkeletonChunk(SkeletonChunk),
    SkeletonBone(SkeletonBone<'a>),
}

const SCHEMA: &str = "ModelSkeletonRecord";
pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 2] = [
    (CHUNK, "skeleton-chunk", 1, SCHEMA),
    (BONE, "skeleton-bone", 2, SCHEMA),
];

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), crate::MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA,
        crate::schema_merge::serialized_payload::<SkeletonRecord<'_>>(),
    )
}
