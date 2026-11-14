// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::mission_text_inputs::{MissionItemGap, MissionKey};
use crate::schema_fields::every_field_required;

/// One authored `MissionItem` token and the outcome of its static text projection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ItemToken {
    pub(super) name: SourcedValue<String>,
    pub(super) variant: SourcedValue<Option<String>>,
    pub(super) outcome: SourcedValue<ItemTokenOutcome>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum ItemTokenOutcome {
    StaticPreview {
        item_guid: CatalogGuid,
        item_record_index: u32,
    },
    Unresolved {
        reason: MissionItemGap,
    },
}

impl ItemToken {
    pub(super) fn new(
        origins: &CatalogOrigins,
        name: &str,
        variant: Option<&str>,
        token_inputs: &[SourceUse],
        outcome_inputs: &[SourceUse],
        outcome: ItemTokenOutcome,
    ) -> Result<Self, CatalogError> {
        let sourced_name = copy(name)?;
        let sourced_variant = variant.map(copy).transpose()?;
        let complete = outcome_inputs
            .iter()
            .all(|source| source.role() != SourceUseRole::Attempted);
        Ok(Self {
            name: origins
                .uses
                .value(SourceValue::Present(sourced_name), token_inputs, true),
            variant: origins
                .uses
                .value(SourceValue::Present(sourced_variant), token_inputs, true),
            outcome: origins
                .uses
                .value(SourceValue::Present(outcome), outcome_inputs, complete),
        })
    }
}

impl ItemTokenOutcome {
    pub(super) const fn preview(guid: Guid, record_index: u32) -> Self {
        Self::StaticPreview {
            item_guid: CatalogGuid(guid),
            item_record_index: record_index,
        }
    }
}

pub(crate) struct SelectedItemKey<'read, 'source> {
    pub(crate) key: &'read MissionKey<'source>,
    pub(crate) guid: Guid,
    pub(crate) record_index: u32,
}

fn copy(value: &str) -> Result<String, CatalogError> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(CatalogError::MissionTextAllocation)?;
    output.push_str(value);
    Ok(output)
}
