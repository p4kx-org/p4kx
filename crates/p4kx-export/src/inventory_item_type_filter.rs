// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "InventoryContainerItemTypeFilter";

/// One authored item-type filter and its ordered subtype enum members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryItemTypeFilter<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    item_type: SourcedValue<&'source str>,
    item_sub_types: SourcedValue<Vec<SourcedValue<&'source str>>>,
}

impl<'source> InventoryItemTypeFilter<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let item_type = source.borrowed_enum_to("itemType", "EItemType", notes)?;
        let item_sub_types =
            source.borrowed_enum_array_to("itemSubTypes", "EItemSubType", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("itemType", item_type.state()),
                ("itemSubTypes", item_sub_types.state()),
            ],
        )?;
        Ok(Self {
            source,
            item_type,
            item_sub_types,
        })
    }
}
