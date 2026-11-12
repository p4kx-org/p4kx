// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::container_layout::ContainerSourceLayout;

pub(in crate::hirc_view) type SwitchSourceSchema =
    SwitchSourceDocument<Vec<SwitchSourceRow>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SwitchObjectSourceFields")]
pub(in crate::hirc_view) struct SwitchSourceDocument<Rows, Inventory> {
    pub(super) kind: SwitchSourceKind,
    pub(super) layout: SourcedValue<ContainerSourceLayout>,
    pub(super) version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<SourcedValue<String>>,
    pub(super) fields: SwitchFields,
    #[schemars(with = "Vec<SwitchSourceRow>")]
    pub(super) rows: Rows,
    #[schemars(with = "SourceInventory")]
    pub(super) source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SwitchSourceKind {
    SwitchContainer,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SwitchFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) group_type: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) group_kind: Option<SourcedValue<SwitchGroupKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) group_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) default_value_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) continuous_validation: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) child_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) groups: Option<SwitchSourceTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) configurations: Option<SwitchSourceTable>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SwitchGroupKind {
    Switch,
    State,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SwitchSourceTable {
    pub(super) declared_count: SourcedValue<u32>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum SwitchSourceRow {
    Group {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        value_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        member_count: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        members_range: Option<SourcedValue<StateRange>>,
        complete: SourcedValue<bool>,
    },
    Member {
        group_ordinal: SourcedValue<u32>,
        ordinal: SourcedValue<u32>,
        child_id: SourcedValue<u32>,
        reference_resolution: SourcedValue<SwitchReferenceState>,
    },
    Configuration {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        child_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        flags: Option<SourcedValue<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out: Option<SwitchFade>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<SwitchFade>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(in crate::hirc_view) enum SwitchReferenceState {
    Unchecked,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct SwitchFade {
    pub(super) bits: SourcedValue<u32>,
    pub(super) signed_value: SourcedValue<i32>,
}
