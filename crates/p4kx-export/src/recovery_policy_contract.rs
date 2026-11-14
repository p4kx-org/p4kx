// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;

pub(crate) type ConditionList<'source> =
    SourcedValue<Vec<SourcedValue<RecoveryCondition<'source>>>>;

/// Authored set conditions and notification settings of the selected physical policy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemRecoveryPolicy<'source> {
    pub(crate) owner_guid: SourcedValue<CatalogGuid>,
    pub(crate) owner_record_index: SourcedValue<u32>,
    #[serde(rename = "Insurable")]
    pub(crate) insurable: ConditionList<'source>,
    #[serde(rename = "ReplenishOnRearm")]
    pub(crate) replenish_on_rearm: ConditionList<'source>,
    #[serde(rename = "ReplenishOnClaimAndRepair")]
    pub(crate) replenish_on_claim_and_repair: ConditionList<'source>,
    #[serde(rename = "NeverRepair")]
    pub(crate) never_repair: ConditionList<'source>,
    #[serde(rename = "NeverLTP")]
    pub(crate) never_ltp: ConditionList<'source>,
    #[serde(rename = "ForceLTP")]
    pub(crate) force_ltp: ConditionList<'source>,
    #[serde(rename = "DontEquipWhenBricked")]
    pub(crate) dont_equip_when_bricked: ConditionList<'source>,
    pub(crate) notification_params: SourcedValue<RecoveryNotifications>,
}

/// One condition occurrence and its selected concrete payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryCondition<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: RecoveryConditionPayload<'source>,
}

/// Known concrete recovery conditions with their original enum or link values.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum RecoveryConditionPayload<'source> {
    Base,
    EntityClass {
        class_def: SourcedValue<CatalogRecordReference>,
    },
    EntityTag {
        tags: SourcedValue<RecoveryConditionTags>,
    },
    ItemType {
        #[serde(rename = "type")]
        item_type: SourcedValue<&'source str>,
        sub_type: SourcedValue<&'source str>,
    },
}

/// An original `TagList` with all ordered reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryConditionTags {
    pub(crate) tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// Authored notification text references and the original notification-buffer scalar.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryNotifications {
    #[serde(rename = "itemBricked_Title")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) item_bricked_title: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricked_Body")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) item_bricked_body: SourcedValue<GameTextRef>,
    #[serde(rename = "vehicleBrickedWithItems_Body")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) vehicle_bricked_with_items_body: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricking_Title")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) item_bricking_title: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricking_Body")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) item_bricking_body: SourcedValue<GameTextRef>,
    #[serde(rename = "vehicleBrickingWithItems_Body")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) vehicle_bricking_with_items_body: SourcedValue<GameTextRef>,
    pub(crate) on_bricked_item_use_notification_buffer: SourcedValue<ResourceFloat32>,
    #[serde(rename = "onBrickedItemUsed_Title")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) on_bricked_item_used_title: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotEquip")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) on_bricked_item_used_cannot_equip: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotFire")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) on_bricked_item_used_cannot_fire: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotUse")]
    #[schemars(extend("x-gameText" = true))]
    pub(crate) on_bricked_item_used_cannot_use: SourcedValue<GameTextRef>,
}
