// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const PROPERTY: &str = "SReputationContextBBPropertyParams";
const BASE: &str = "SBBDynamicPropertyBase";

/// An authored reputation blackboard property and its original dynamic value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct BlackboardProperty<'source> {
    source_type: SourcedValue<&'source str>,
    name: SourcedValue<&'source str>,
    dynamic_property: SourcedValue<BlackboardPropertyBody<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> BlackboardProperty<'source> {
    /// Reads the original ordered property slots under their nominal Class declaration.
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<Self>>>, CatalogError> {
        fields.class_array_to("propertiesBB", PROPERTY, notes, |property, notes| {
            Self::read(property, projection, notes)
        })
    }

    /// Reads a property at its original position.
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.borrowed_string("name", notes)?;
        let dynamic_property =
            fields.strong_pointer_family_to("dynamicProperty", BASE, notes, |body, notes| {
                BlackboardPropertyBody::read(body, projection, notes)
            })?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("dynamicProperty", dynamic_property.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            name,
            dynamic_property,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct BlackboardPropertyBody<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    value: BlackboardValue<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> BlackboardPropertyBody<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let value = BlackboardValue::read(fields, projection, notes)?;
        let projected = value.state().map(|state| ("value", state));
        Ok(Self {
            source_type: fields.source_type_value()?,
            value,
            authored_fields: fields.authored_members_except(projected)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum BlackboardValue<'source> {
    Boolean {
        value: SourcedValue<bool>,
    },
    Float {
        value: SourcedValue<ResourceFloat32>,
    },
    Integer {
        value: SourcedValue<i32>,
    },
    Localized {
        #[schemars(extend("x-gameText" = true))]
        value: SourcedValue<GameTextRef>,
    },
    String {
        value: SourcedValue<&'source str>,
    },
    Authored,
}

impl<'source> BlackboardValue<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match fields.source_type()? {
            "SBBDynamicPropertyBool" => Self::Boolean {
                value: fields.boolean("value", notes)?,
            },
            "SBBDynamicPropertyFloat" => Self::Float {
                value: fields.single("value", notes)?,
            },
            "SBBDynamicPropertyInt" => Self::Integer {
                value: fields.int32("value", notes)?,
            },
            "SBBDynamicPropertyLocString" => Self::Localized {
                value: fields.protected_locale("value", projection, notes)?,
            },
            "SBBDynamicPropertyString" => Self::String {
                value: fields.borrowed_string("value", notes)?,
            },
            BASE => Self::Authored,
            _ => {
                fields.note_retained_variant(notes)?;
                Self::Authored
            }
        })
    }

    const fn state(&self) -> Option<SourceValueState> {
        Some(match self {
            Self::Boolean { value } => value.state(),
            Self::Float { value } => value.state(),
            Self::Integer { value } => value.state(),
            Self::Localized { value } => value.state(),
            Self::String { value } => value.state(),
            Self::Authored => return None,
        })
    }
}
