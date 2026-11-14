// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue, Value};

use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::source_fields::{FieldCapture, FieldError};

use super::contract::{
    PropertyNameOverrideConditionPayload, PropertyNameOverrideConditionSource,
    PropertyNameOverrideFields, PropertyNameOverrideSource,
};
use super::read as source_read;

/// Preserves every original inline override slot without selecting a winning name.
pub(crate) fn read<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<PropertyNameOverrideSource<'source>>>>, CatalogError> {
    fields.array("nameOverrides", notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let path = fields.location(&format!("nameOverrides[{index}]"));
                fields
                    .structure_element(&format!("nameOverrides[{index}]"), value, notes)?
                    .try_map(|source| {
                        entry(
                            source,
                            &path,
                            fields.context(),
                            record_index,
                            projection,
                            capture,
                            notes,
                        )
                    })
            })
            .collect()
    })
}

fn entry<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<PropertyNameOverrideSource<'source>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let source_type = fields.source_type()?;
    let payload = if source.type_name() == "CraftingPropertyNameOverride" {
        fields.check_observed(&["propertyName", "condition"], notes)?;
        Some(PropertyNameOverrideFields {
            property_name: source_read::name(
                &fields,
                "propertyName",
                record_index,
                projection,
                notes,
            )?,
            condition: fields.structure("condition", notes)?.try_map(|source| {
                condition(
                    source,
                    &fields.location("condition"),
                    context,
                    capture,
                    notes,
                )
            })?,
        })
    } else {
        fields.check_observed(&[], notes)?;
        None
    };
    let payload = source_read::payload(&fields, payload, notes)?;
    Ok(PropertyNameOverrideSource {
        source_type,
        payload,
    })
}

fn condition<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<PropertyNameOverrideConditionSource<'source>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let source_type = fields.source_type()?;
    let (payload, known): (_, &[&str]) = match source.type_name() {
        "CraftingPropertyNameOverrideCondition_Base"
        | "CraftingPropertyNameOverrideCondition_Base_NonRef" => {
            (Some(PropertyNameOverrideConditionPayload::EmptyBase), &[])
        }
        "CraftingPropertyNameOverrideCondition_ItemType" => (
            Some(PropertyNameOverrideConditionPayload::ItemType {
                match_item_types: labels(&fields, "matchItemTypes", notes)?,
                match_sub_types: labels(&fields, "matchSubTypes", notes)?,
            }),
            &["matchItemTypes", "matchSubTypes"],
        ),
        _ => (None, &[]),
    };
    fields.check_observed(known, notes)?;
    let payload = source_read::payload(&fields, payload, notes)?;
    Ok(PropertyNameOverrideConditionSource {
        source_type,
        payload,
    })
}

fn labels<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<&'source str>>>, CatalogError> {
    fields.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let name = format!("{name}[{index}]");
                fields.read_value(
                    Some(value),
                    &name,
                    "a nullable Enum",
                    notes,
                    |value| match value {
                        Value::Enum(value) => Some(*value),
                        _ => None,
                    },
                )
            })
            .collect()
    })
}
