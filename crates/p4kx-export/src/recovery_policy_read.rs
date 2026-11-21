// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceValue, SourcedValue, Value};

use super::economy::ECONOMY;
use super::notification_read;
use super::overrides;
use super::policy_contract::{
    ConditionList, ItemRecoveryPolicy, RecoveryCondition, RecoveryConditionPayload,
    RecoveryConditionTags,
};
use super::projection::RecoveryReader;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::source_fields::FieldError;

const BASE: &str = "ItemRecoverySetConditionDef";
const ENTITY_CLASS: &str = "ItemRecoveryCondition_EntityClass";
const ENTITY_TAG: &str = "ItemRecoveryCondition_EntityTag";
const ITEM_TYPE: &str = "ItemRecoveryCondition_ItemType";
const NOTIFICATIONS: &str = "notificationParams";

/// Reads the original policy siblings independently of the economy projection.
pub(crate) fn read<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    root: &CatalogFields<'_, 'source, '_>,
    source: &CanonicalRecordHeader<'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ItemRecoveryPolicy<'source>, CatalogError> {
    root.check_observed(
        &[
            ECONOMY,
            NOTIFICATIONS,
            "Insurable",
            "ReplenishOnRearm",
            "ReplenishOnClaimAndRepair",
            "NeverRepair",
            "NeverLTP",
            "ForceLTP",
            "DontEquipWhenBricked",
        ],
        notes,
    )?;
    Ok(ItemRecoveryPolicy {
        owner_guid: source.guid.clone(),
        owner_record_index: source.record_index.clone(),
        insurable: conditions(reader, root, "Insurable", notes)?,
        replenish_on_rearm: conditions(reader, root, "ReplenishOnRearm", notes)?,
        replenish_on_claim_and_repair: conditions(
            reader,
            root,
            "ReplenishOnClaimAndRepair",
            notes,
        )?,
        never_repair: conditions(reader, root, "NeverRepair", notes)?,
        never_ltp: conditions(reader, root, "NeverLTP", notes)?,
        force_ltp: conditions(reader, root, "ForceLTP", notes)?,
        dont_equip_when_bricked: conditions(reader, root, "DontEquipWhenBricked", notes)?,
        notification_params: reader.block(
            root,
            NOTIFICATIONS,
            "ItemRecoveryNotificationParams",
            notes,
            |fields, notes| notification_read::read(fields, projection, notes),
        )?,
    })
}

fn conditions<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    root: &CatalogFields<'_, 'source, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ConditionList<'source>, CatalogError> {
    root.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                if reader.cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                let indexed = format!("{name}[{ordinal}]");
                let selected = root.structure_element(&indexed, value, notes)?;
                let path = root.location(&indexed);
                condition(reader, selected, &path, notes)
            })
            .collect()
    })
}

fn condition<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    selected: SourcedValue<&p4kx_extraction::StructValue<'source>>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<RecoveryCondition<'source>>, CatalogError> {
    if let Some(source) = selected.value()
        && !matches!(
            source.type_name(),
            BASE | ENTITY_CLASS | ENTITY_TAG | ITEM_TYPE
        )
    {
        let fields = CatalogFields::observed(source, path, reader.context, reader.capture)?;
        fields.note_unsupported(notes)?;
        return reader
            .context
            .selector(&fields.location("$type"), SourceValue::Unsupported);
    }
    selected.try_map(|source| {
        let fields = CatalogFields::observed(source, path, reader.context, reader.capture)?;
        let payload = condition_payload(reader, &fields, notes)?;
        Ok(RecoveryCondition {
            source_type: fields.source_type()?,
            payload,
        })
    })
}

fn condition_payload<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    fields: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryConditionPayload<'source>, CatalogError> {
    let payload = match fields.source().type_name() {
        BASE => {
            fields.check_observed(&[], notes)?;
            RecoveryConditionPayload::Base
        }
        ENTITY_CLASS => {
            fields.check_observed(&["classDef"], notes)?;
            RecoveryConditionPayload::EntityClass {
                class_def: fields.reference("classDef", notes)?,
            }
        }
        ENTITY_TAG => {
            fields.check_observed(&["tags"], notes)?;
            RecoveryConditionPayload::EntityTag {
                tags: reader.block(fields, "tags", "TagList", notes, tag_list)?,
            }
        }
        ITEM_TYPE => {
            fields.check_observed(&["type", "subType"], notes)?;
            RecoveryConditionPayload::ItemType {
                item_type: enum_value(fields, "type", notes)?,
                sub_type: enum_value(fields, "subType", notes)?,
            }
        }
        _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok(payload)
}

fn tag_list(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryConditionTags, CatalogError> {
    fields.check_observed(&["tags"], notes)?;
    Ok(RecoveryConditionTags {
        tags: overrides::references(fields, "tags", notes)?,
    })
}

fn enum_value<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<&'source str>, CatalogError> {
    fields.read(name, "a nullable Enum", notes, |value| match value {
        Value::Enum(value) => Some(*value),
        _ => None,
    })
}
