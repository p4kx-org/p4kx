// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum NodePlaybackPayload<'row> {
    #[serde(rename = "bank-hirc-node-positioning")]
    Positioning(&'row NodeDeclaration<PositioningObservations>),
    #[serde(rename = "bank-hirc-node-positioning-path")]
    Path(&'row PositioningPathRecord),
    #[serde(rename = "bank-hirc-node-advanced-settings")]
    Advanced(&'row NodeDeclaration<AdvancedObservations>),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PositioningObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) modes: Option<PositioningModes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) spatial_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path_mode: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_time: Option<BankField<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) vertex_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) playlist_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) vertices_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) playlist_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) automation_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PositioningModes {
    pub(crate) override_parent: bool,
    pub(crate) has_spatial_fields: bool,
    pub(crate) panner_type_code: u8,
    pub(crate) position_type_code: u8,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdvancedObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) virtual_queue_behavior: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_instances: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) below_threshold_behavior: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) behavior_flags: Option<BankField<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PositioningRowLayout {
    Decoded,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PositioningPathRecord {
    pub(crate) layout: PositioningRowLayout,
    pub(crate) ordinal: u32,
    pub(crate) fields: PositioningPathFields,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum PositioningPathFields {
    Vertex {
        x: PositioningCoordinate,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<PositioningCoordinate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        z: Option<PositioningCoordinate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<BankField<i32>>,
    },
    Playlist {
        vertices_offset: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        vertex_count: Option<BankField<u32>>,
    },
    AutomationRange {
        x: PositioningCoordinate,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<PositioningCoordinate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        z: Option<PositioningCoordinate>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PositioningCoordinate {
    pub(crate) bits: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finite_value: Option<f32>,
}
