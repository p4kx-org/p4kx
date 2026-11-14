// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_source::{InteractionList, RecordLinks, original};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original condition presets and both ordered authored predicate lists.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionConditions<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    preset_list: RecordLinks<'source>,
    condition_params: InteractionList<UnitSource<'source>>,
    condition_to_hide_params: InteractionList<UnitSource<'source>>,
}

impl<'source> InteractionConditions<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let preset_list =
            source.record_link_array_to("presetList", "InteractionConditionPreset", notes)?;
        let condition_params = source.strong_pointer_family_array_to(
            "conditionParams",
            "InteractionConditionParams",
            notes,
            original,
        )?;
        let condition_to_hide_params = source.strong_pointer_family_array_to(
            "conditionToHideParams",
            "InteractionConditionParams",
            notes,
            original,
        )?;
        let source = UnitSource::read(
            source,
            [
                ("presetList", preset_list.state()),
                ("conditionParams", condition_params.state()),
                ("conditionToHideParams", condition_to_hide_params.state()),
            ],
        )?;
        Ok(Self {
            source,
            preset_list,
            condition_params,
            condition_to_hide_params,
        })
    }
}
