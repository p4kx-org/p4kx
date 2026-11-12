// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::container_layout::ContainerSourceLayout;

pub(in crate::hirc_view) type BlendSourceSchema =
    BlendSourceDocument<Vec<BlendSourceRow>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "BlendObjectSourceFields")]
pub(in crate::hirc_view) struct BlendSourceDocument<Rows, Inventory> {
    pub(super) kind: BlendSourceKind,
    pub(super) layout: SourcedValue<ContainerSourceLayout>,
    pub(super) version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<SourcedValue<String>>,
    pub(super) fields: BlendFields,
    #[schemars(with = "Vec<BlendSourceRow>")]
    pub(super) rows: Rows,
    #[schemars(with = "SourceInventory")]
    pub(super) source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BlendSourceKind {
    BlendContainer,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BlendFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) layers: Option<BlendSourceTable<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) continuous_validation: Option<SourcedValue<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "BlendSourceTable_{Count}")]
pub(in crate::hirc_view) struct BlendSourceTable<Count> {
    pub(super) declared_count: SourcedValue<Count>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum BlendSourceRow {
    Child {
        ordinal: SourcedValue<u32>,
        child_id: SourcedValue<u32>,
    },
    Layer {
        layer_ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        layer_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        initial_rtpc: Option<BlendSourceTable<u16>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        crossfade_rtpc_id: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        crossfade_rtpc_type: Option<SourcedValue<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        associations: Option<BlendSourceTable<u32>>,
        complete: SourcedValue<bool>,
    },
    RtpcCurve {
        layer_ordinal: SourcedValue<u32>,
        curve_ordinal: SourcedValue<u16>,
        rtpc_id: SourcedValue<u32>,
        rtpc_type: SourcedValue<u8>,
        accumulation: SourcedValue<u8>,
        parameter_id: SourcedValue<u32>,
        curve_id: SourcedValue<u32>,
        scaling: SourcedValue<u8>,
        point_count: SourcedValue<u16>,
    },
    RtpcPoint {
        layer_ordinal: SourcedValue<u32>,
        curve_ordinal: SourcedValue<u16>,
        point_ordinal: SourcedValue<u16>,
        from_bits: SourcedValue<u32>,
        to_bits: SourcedValue<u32>,
        interpolation: SourcedValue<u32>,
    },
    Association {
        layer_ordinal: SourcedValue<u32>,
        association_ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        child_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        points: Option<BlendSourceTable<u32>>,
        complete: SourcedValue<bool>,
        reference_resolution: SourcedValue<BlendReferenceState>,
    },
    CrossfadePoint {
        layer_ordinal: SourcedValue<u32>,
        association_ordinal: SourcedValue<u32>,
        point_ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        from_bits: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to_bits: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interpolation: Option<SourcedValue<u32>>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(in crate::hirc_view) enum BlendReferenceState {
    Unchecked,
}
