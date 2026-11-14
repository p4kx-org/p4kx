// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::PublicNameRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::source_value_catalog::SourceRecordReference;

pub(crate) const SOURCE_TYPE: &str = "StarMapAmenityTypeEntry";

/// One physical starmap amenity and its authored required entity link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StarmapAmenityDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    name: SourcedValue<&'source str>,
    display_name: SourcedValue<PublicNameRef>,
    icon: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    required_entity: SourcedValue<Box<SourceRecordReference<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapAmenityDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.borrowed_string("name", notes)?;
        let display_name = fields.public_name("displayName", projection, notes)?;
        let icon = fields.borrowed_string("icon", notes)?;
        let required_entity =
            fields.record_link_to("requiredEntity", "LocationEntityDeclaration", notes)?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("displayName", display_name.state()),
            ("icon", icon.state()),
            ("requiredEntity", required_entity.state()),
        ])?;
        Ok(Self {
            source,
            name,
            display_name,
            icon,
            required_entity,
            authored_fields,
        })
    }
}
