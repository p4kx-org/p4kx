// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::infraction_parameters::InfractionParameters;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const DEFINITION_TYPE: &str = "InfractionDefinition";
pub(crate) const SET_TYPE: &str = "InfractionSet";
const USE_TYPE: &str = "Infraction";
const TRIGGER_TYPE: &str = "EInfractionTrigger";

/// One physical definition of an authored infraction.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InfractionDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<GameTextRef>,
    triggers: SourcedValue<Vec<SourcedValue<&'source str>>>,
    default_parameters: SourcedValue<InfractionParameters<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> InfractionDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.public_name("name", projection, notes)?;
        let description = fields.protected_locale("description", projection, notes)?;
        let triggers = fields.borrowed_enum_array_to("triggers", TRIGGER_TYPE, notes)?;
        let default_parameters =
            InfractionParameters::read_field(fields, "defaultParameters", projection, notes)?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("description", description.state()),
            ("triggers", triggers.state()),
            ("defaultParameters", default_parameters.state()),
        ])?;
        Ok(Self {
            source,
            name,
            description,
            triggers,
            default_parameters,
            authored_fields,
        })
    }
}

/// One reusable physical set of ordered infraction uses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InfractionSetDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    infractions: SourcedValue<Vec<SourcedValue<InfractionUse<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> InfractionSetDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let infractions = InfractionUse::read_array(fields, projection, notes)?;
        let authored_fields =
            fields.authored_members_except([("infractions", infractions.state())])?;
        Ok(Self {
            source,
            infractions,
            authored_fields,
        })
    }
}

/// A declared infraction link with its own parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InfractionUse<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    definition: SourcedValue<Box<SourceRecordReference<'source>>>,
    parameter_overrides: SourcedValue<InfractionParameters<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> InfractionUse<'source> {
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<Self>>>, CatalogError> {
        fields.class_array_to("infractions", USE_TYPE, notes, |owned, notes| {
            Self::read(owned, projection, notes)
        })
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let definition = fields.record_link_to("definition", DEFINITION_TYPE, notes)?;
        let parameter_overrides =
            InfractionParameters::read_field(fields, "parameterOverrides", projection, notes)?;
        let authored_fields = fields.authored_members_except([
            ("definition", definition.state()),
            ("parameterOverrides", parameter_overrides.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            definition,
            parameter_overrides,
            authored_fields,
        })
    }
}
