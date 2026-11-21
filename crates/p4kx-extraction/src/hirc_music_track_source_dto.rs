// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

pub(in crate::hirc_view) type MusicTrackSourceSchema =
    MusicTrackSourceDocument<Vec<MusicTrackSourceRow>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MusicTrackObjectSourceFields")]
pub(in crate::hirc_view) struct MusicTrackSourceDocument<Rows, Inventory> {
    pub(super) kind: MusicTrackSourceKind,
    pub(super) layout: SourcedValue<MusicTrackSourceLayout>,
    pub(super) version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<SourcedValue<String>>,
    pub(super) fields: MusicTrackSourceFields,
    #[schemars(with = "Vec<MusicTrackSourceRow>")]
    pub(super) rows: Rows,
    #[schemars(with = "SourceInventory")]
    pub(super) source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MusicTrackSourceKind {
    MusicTrack,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MusicTrackSourceLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MusicTrackSourceFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) clip_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) subtrack_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) parent_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) override_bus_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) node_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) kind_code: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) look_ahead: Option<MusicTrackSignedWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) automation: Option<MusicTrackSourceTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) switch: Option<MusicTrackSourceSwitch>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MusicTrackSourceSwitch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) group_type: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) group_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) default_value_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) member_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition: Option<MusicTrackSourceTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_range: Option<SourcedValue<StateRange>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MusicTrackSourceTransition {
    pub(super) source_fade: MusicTrackSourceFade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sync_type: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cue_filter_hash: Option<SourcedValue<u32>>,
    pub(super) destination_fade: MusicTrackSourceFade,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MusicTrackSourceFade {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) time: Option<MusicTrackSignedWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) curve: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) offset: Option<MusicTrackSignedWord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MusicTrackSignedWord {
    pub(super) bits: SourcedValue<u32>,
    pub(super) signed_value: SourcedValue<i32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct MusicTrackBinary64Value {
    pub(super) bits: SourcedValue<Binary64Word>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) finite_value: Option<SourcedValue<f64>>,
}

pub(super) struct Binary64Word(pub(super) u64);

impl Serialize for Binary64Word {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("{:016x}", self.0))
    }
}

impl JsonSchema for Binary64Word {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("WwiseBinary64Word")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({ "type": "string", "pattern": "^[0-9a-f]{16}$" })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct MusicTrackSourceTable {
    pub(super) declared_count: SourcedValue<u32>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum MusicTrackSourceRow {
    MediaSource {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_id: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        storage_code: Option<SourcedValue<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_id: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_id: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        in_memory_size: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_bits: Option<SourcedValue<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_parameter_length: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_parameters_range: Option<SourcedValue<StateRange>>,
    },
    Clip {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        track_id: SourcedValue<u32>,
        source_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_id: Option<SourcedValue<u32>>,
        event_id: SourcedValue<u32>,
        play_at: MusicTrackBinary64Value,
        begin_trim: MusicTrackBinary64Value,
        end_trim: MusicTrackBinary64Value,
        source_duration: MusicTrackBinary64Value,
    },
    Automation {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        clip_index: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        automation_type: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        points: Option<MusicTrackSourceTable>,
    },
    AutomationPoint {
        automation_ordinal: SourcedValue<u32>,
        point_ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        from_bits: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to_bits: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interpolation: Option<SourcedValue<u32>>,
    },
    SwitchMember {
        ordinal: SourcedValue<u32>,
        value_id: SourcedValue<u32>,
    },
}
