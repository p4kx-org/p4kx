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

/// Original condition slots in source order with their individual source states.
pub type ConditionList<'source> = SourcedValue<Vec<SourcedValue<RecoveryCondition<'source>>>>;

/// Authored set conditions and notification settings of the selected physical policy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ItemRecoveryPolicy<'source> {
    /// The original owner's GUID.
    pub owner_guid: SourcedValue<CatalogGuid>,
    /// The original owner's physical Record index.
    pub owner_record_index: SourcedValue<u32>,
    #[serde(rename = "Insurable")]
    /// Original insurability conditions.
    pub insurable: ConditionList<'source>,
    #[serde(rename = "ReplenishOnRearm")]
    /// Original rearm replenishment conditions.
    pub replenish_on_rearm: ConditionList<'source>,
    #[serde(rename = "ReplenishOnClaimAndRepair")]
    /// Original claim and repair replenishment conditions.
    pub replenish_on_claim_and_repair: ConditionList<'source>,
    #[serde(rename = "NeverRepair")]
    /// Original repair exclusion conditions.
    pub never_repair: ConditionList<'source>,
    #[serde(rename = "NeverLTP")]
    /// Original persistence exclusion conditions.
    pub never_ltp: ConditionList<'source>,
    #[serde(rename = "ForceLTP")]
    /// Original required persistence conditions.
    pub force_ltp: ConditionList<'source>,
    #[serde(rename = "DontEquipWhenBricked")]
    /// Original conditions excluding bricked items from equipment.
    pub dont_equip_when_bricked: ConditionList<'source>,
    /// The authored notification references and buffer.
    pub notification_params: SourcedValue<RecoveryNotifications>,
}

/// One condition occurrence and its selected concrete payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RecoveryCondition<'source> {
    /// The original concrete condition type.
    pub source_type: SourcedValue<&'source str>,
    /// The typed payload of the original condition.
    pub payload: RecoveryConditionPayload<'source>,
}

/// Known concrete recovery conditions with their original enum or link values.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum RecoveryConditionPayload<'source> {
    /// The base condition has no additional fields.
    Base,
    /// A condition using an authored Entity class reference.
    EntityClass {
        /// The original class reference and target header evidence.
        class_def: SourcedValue<CatalogRecordReference>,
    },
    /// A condition using a list of Entity tags.
    EntityTag {
        /// The original tag list.
        tags: SourcedValue<RecoveryConditionTags>,
    },
    /// A condition matching the authored item type and subtype.
    ItemType {
        /// The original item-type Enum value.
        #[serde(rename = "type")]
        item_type: SourcedValue<&'source str>,
        /// The original subtype Enum value.
        sub_type: SourcedValue<&'source str>,
    },
}

/// An original `TagList` with all ordered reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RecoveryConditionTags {
    /// The original tag References in source order.
    pub tags: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// Authored notification text references and the original notification-buffer scalar.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RecoveryNotifications {
    #[serde(rename = "itemBricked_Title")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored title for a bricked-item notification.
    pub item_bricked_title: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricked_Body")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored body for a bricked-item notification.
    pub item_bricked_body: SourcedValue<GameTextRef>,
    #[serde(rename = "vehicleBrickedWithItems_Body")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored body for a bricked vehicle still holding items.
    pub vehicle_bricked_with_items_body: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricking_Title")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored title for an item-bricking notification.
    pub item_bricking_title: SourcedValue<GameTextRef>,
    #[serde(rename = "itemBricking_Body")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored body for an item-bricking notification.
    pub item_bricking_body: SourcedValue<GameTextRef>,
    #[serde(rename = "vehicleBrickingWithItems_Body")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored body for a bricking vehicle still holding items.
    pub vehicle_bricking_with_items_body: SourcedValue<GameTextRef>,
    /// The authored notification buffer for attempted bricked-item use.
    pub on_bricked_item_use_notification_buffer: SourcedValue<ResourceFloat32>,
    #[serde(rename = "onBrickedItemUsed_Title")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored title for a bricked-item use notification.
    pub on_bricked_item_used_title: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotEquip")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored message for a bricked item that cannot be equipped.
    pub on_bricked_item_used_cannot_equip: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotFire")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored message for a bricked item that cannot fire.
    pub on_bricked_item_used_cannot_fire: SourcedValue<GameTextRef>,
    #[serde(rename = "onBrickedItemUsed_CannotUse")]
    #[schemars(extend("x-gameText" = true))]
    /// The authored message for a bricked item that cannot be used.
    pub on_bricked_item_used_cannot_use: SourcedValue<GameTextRef>,
}
