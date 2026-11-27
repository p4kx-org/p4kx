// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::jurisdiction_definition;
use super::starmap_amenity_definition;
use super::starmap_object_definition::SOURCE_TYPE;
use super::starmap_type_definition;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// Original starmap object relationships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct StarmapObjectLinks<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original legacy faction reference.
    pub affiliation: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original jurisdiction reference.
    pub jurisdiction: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original location hierarchy tag reference.
    pub location_hierarchy_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original starmap object type reference.
    pub r#type: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original starmap parent reference.
    pub parent: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    /// The original amenity references in stored order.
    pub amenities: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
}

impl<'source> StarmapObjectLinks<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            affiliation: fields.record_link_to("affiliation", "Faction_LEGACY", notes)?,
            jurisdiction: fields.record_link_to(
                "jurisdiction",
                jurisdiction_definition::SOURCE_TYPE,
                notes,
            )?,
            location_hierarchy_tag: fields.record_link_to("locationHierarchyTag", "Tag", notes)?,
            r#type: fields.record_link_to("type", starmap_type_definition::SOURCE_TYPE, notes)?,
            parent: Self::parent(fields, notes)?,
            amenities: fields.record_link_array_to(
                "amenities",
                starmap_amenity_definition::SOURCE_TYPE,
                notes,
            )?,
        })
    }

    pub(super) fn parent(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Box<SourceRecordReference<'source>>>, CatalogError> {
        fields.record_link_to("parent", SOURCE_TYPE, notes)
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("affiliation", self.affiliation.state()),
            ("jurisdiction", self.jurisdiction.state()),
            ("locationHierarchyTag", self.location_hierarchy_tag.state()),
            ("type", self.r#type.state()),
            ("parent", self.parent.state()),
            ("amenities", self.amenities.state()),
        ]
    }
}
