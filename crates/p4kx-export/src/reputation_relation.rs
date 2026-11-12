// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::faction_definition::FactionText;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const SOURCE_TYPE: &str = "RelationStandingParams";
const MARKER: &str = "RelationMarkerParams";
const SCOPE: &str = "SReputationScopeParams";
const STANDING: &str = "SReputationStandingParams";

/// An original allied or hostile standing rule with its authored marker.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ReputationRelation<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    scope: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    standing: SourcedValue<Box<SourceRecordReference<'source>>>,
    marker_params: SourcedValue<RelationMarker<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ReputationRelation<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let scope = fields.record_link_to("scope", SCOPE, notes)?;
        let standing = fields.record_link_to("standing", STANDING, notes)?;
        let marker_params =
            fields.declared_class_to("markerParams", MARKER, notes, |marker, notes| {
                RelationMarker::read(marker, projection, notes)
            })?;
        let authored_fields = fields.authored_members_except([
            ("scope", scope.state()),
            ("standing", standing.state()),
            ("markerParams", marker_params.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            scope,
            standing,
            marker_params,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RelationMarker<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<FactionText<'source, GameTextRef>>,
    icon: SourcedValue<&'source str>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> RelationMarker<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let description = FactionText::read(fields, "description", notes, |fact| {
            projection.protected_locale_fact(fact)
        })?;
        let icon = fields.borrowed_string("icon", notes)?;
        let authored_fields = fields.authored_members_except([
            ("description", description.state()),
            ("icon", icon.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            description,
            icon,
            authored_fields,
        })
    }
}
