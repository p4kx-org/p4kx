// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;

use crate::Reference;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "AnimationSkeletonDisposition")]
pub(crate) enum SkeletonDisposition {
    Ready {
        chunk_ordinal: usize,
        bone_count: usize,
    },
    Missing,
    Ambiguous,
    ExplicitlyExcluded,
    Unavailable,
    NoSupportedSkeleton,
    InvalidContainer,
    InvalidSkeleton,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationSkeleton")]
pub(crate) struct Skeleton<'a> {
    #[schemars(schema_with = "reference_schema")]
    pub(crate) reference: &'a Reference,
    pub(crate) skeleton: SkeletonDisposition,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationBindingDisposition")]
pub(crate) enum BindingDisposition {
    Bound,
    EmptyController,
    MissingBone,
    DuplicateController,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnimationControllerBinding")]
#[schemars(extend("allOf" = [
    {
        "if": {"properties": {"disposition": {"const": "bound"}}, "required": ["disposition"]},
        "then": {"properties": {"bound_bone_ordinal": {"type": "integer"}}},
        "else": {"properties": {"bound_bone_ordinal": {"type": "null"}}}
    },
    {
        "if": {"properties": {"matching_bones": {"const": 0}}, "required": ["matching_bones"]},
        "then": {"properties": {"first_bone_ordinal": {"type": "null"}}},
        "else": {"properties": {"first_bone_ordinal": {"type": "integer"}}}
    }
]))]
pub(crate) struct Binding {
    pub(crate) disposition: BindingDisposition,
    pub(crate) first_controller_ordinal: usize,
    pub(crate) first_track_ordinal: Option<usize>,
    pub(crate) first_bone_ordinal: Option<usize>,
    pub(crate) matching_bones: usize,
    pub(crate) bound_bone_ordinal: Option<usize>,
}

fn reference_schema(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({"$ref": "#/$defs/Reference"})
}
