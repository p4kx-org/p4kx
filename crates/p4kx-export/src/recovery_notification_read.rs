// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::economy;
use super::policy_contract::RecoveryNotifications;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::source_fields::FieldError;

pub(crate) fn read<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryNotifications, CatalogError> {
    fields.check_observed(
        &[
            "itemBricked_Title",
            "itemBricked_Body",
            "vehicleBrickedWithItems_Body",
            "itemBricking_Title",
            "itemBricking_Body",
            "vehicleBrickingWithItems_Body",
            "onBrickedItemUseNotificationBuffer",
            "onBrickedItemUsed_Title",
            "onBrickedItemUsed_CannotEquip",
            "onBrickedItemUsed_CannotFire",
            "onBrickedItemUsed_CannotUse",
        ],
        notes,
    )?;
    let index = fields
        .context()
        .record_position()
        .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
    let text = |name: &str, notes: &mut Vec<FieldError>| {
        let key = fields.locale(name, notes)?;
        projection.description(key, index, &fields.location(name))
    };
    Ok(RecoveryNotifications {
        item_bricked_title: text("itemBricked_Title", notes)?,
        item_bricked_body: text("itemBricked_Body", notes)?,
        vehicle_bricked_with_items_body: text("vehicleBrickedWithItems_Body", notes)?,
        item_bricking_title: text("itemBricking_Title", notes)?,
        item_bricking_body: text("itemBricking_Body", notes)?,
        vehicle_bricking_with_items_body: text("vehicleBrickingWithItems_Body", notes)?,
        on_bricked_item_use_notification_buffer: economy::single(
            fields,
            "onBrickedItemUseNotificationBuffer",
            notes,
        )?,
        on_bricked_item_used_title: text("onBrickedItemUsed_Title", notes)?,
        on_bricked_item_used_cannot_equip: text("onBrickedItemUsed_CannotEquip", notes)?,
        on_bricked_item_used_cannot_fire: text("onBrickedItemUsed_CannotFire", notes)?,
        on_bricked_item_used_cannot_use: text("onBrickedItemUsed_CannotUse", notes)?,
    })
}
