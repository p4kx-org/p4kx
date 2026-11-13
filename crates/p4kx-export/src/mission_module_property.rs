// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// One original `MissionProperty` occurrence owned by a module.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct ModuleMissionProperty<'source> {
    mission_variable_name: SourcedValue<&'source str>,
    extended_text_token: SourcedValue<&'source str>,
    value: SourcedValue<ModuleMissionPropertyValue<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ModuleMissionProperty<'source> {
    pub(in crate::catalog_v3) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mission_variable_name = fields.borrowed_string("missionVariableName", notes)?;
        let extended_text_token = fields.borrowed_string("extendedTextToken", notes)?;
        let value = fields.strong_pointer_family_to(
            "value",
            "BaseMissionPropertyValue",
            notes,
            ModuleMissionPropertyValue::read,
        )?;
        let authored_fields = fields.authored_members_except([
            ("missionVariableName", mission_variable_name.state()),
            ("extendedTextToken", extended_text_token.state()),
            ("value", value.state()),
        ])?;
        Ok(Self {
            mission_variable_name,
            extended_text_token,
            value,
            authored_fields,
        })
    }
}

/// A concrete property payload in its original module source scope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ModuleMissionPropertyValue<'source> {
    source_type: SourcedValue<&'source str>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ModuleMissionPropertyValue<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        _notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: fields.source_type_value()?,
            authored_fields: fields.authored_members()?,
        })
    }
}
