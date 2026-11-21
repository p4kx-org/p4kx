// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::container_layout::ContainerSourceLayout;

pub(in crate::hirc_view) type RandomSequenceSchema =
    RandomSequenceDocument<Vec<RandomSequenceRow>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RandomSequenceObjectSourceFields")]
pub(in crate::hirc_view) struct RandomSequenceDocument<Rows, Inventory> {
    pub(super) kind: RandomSequenceSourceKind,
    pub(super) layout: SourcedValue<ContainerSourceLayout>,
    pub(super) version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<SourcedValue<String>>,
    pub(super) fields: RandomSequenceFields,
    #[schemars(with = "Vec<RandomSequenceRow>")]
    pub(super) rows: Rows,
    #[schemars(with = "SourceInventory")]
    pub(super) source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RandomSequenceSourceKind {
    RandomSequenceContainer,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RandomSequenceFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) loop_count: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) loop_minimum: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) loop_maximum: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_time: Option<RandomSequenceTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_minimum: Option<RandomSequenceTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_maximum: Option<RandomSequenceTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) avoid_repeat_count: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_mode: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) random_mode: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mode: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) playlist: Option<RandomSequencePlaylist>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RandomSequenceTransition {
    pub(super) bits: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) finite_value: Option<SourcedValue<f32>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RandomSequencePlaylist {
    pub(super) declared_count: SourcedValue<u16>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct RandomSequenceRow {
    pub(super) ordinal: SourcedValue<u16>,
    pub(super) range: SourcedValue<StateRange>,
    pub(super) play_id: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) weight_bits: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) weight: Option<SourcedValue<i32>>,
}
