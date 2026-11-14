// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const COMPONENT: &str = "LootGenerationComponentParams";

/// Original loot generation configuration of one physical Entity component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LootGenerationComponent<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LootGenerationComponent<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
        _: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: observed.source_type_value()?,
            authored_fields: observed.authored_members()?,
        })
    }
}
