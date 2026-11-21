// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const SOURCE_TYPE: &str = "LocationResourceSlot";

/// A physical resource slot declaration referenced by mission modules.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LocationResourceSlot<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    number_of_resource_slots_per_entity: SourcedValue<i32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource_slot_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> LocationResourceSlot<'source> {
    /// Borrows the physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the authored number of slots per entity.
    #[must_use]
    pub const fn number_of_resource_slots_per_entity(&self) -> &SourcedValue<i32> {
        &self.number_of_resource_slots_per_entity
    }

    /// Borrows the original resource slot Tag Reference.
    #[must_use]
    pub const fn resource_slot_tag(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.resource_slot_tag
    }

    /// Borrows additional authored fields.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let number_of_resource_slots_per_entity =
            fields.int32("numberOfResourceSlotsPerEntity", notes)?;
        let resource_slot_tag = fields.record_link_to("resourceSlotTag", "Tag", notes)?;
        let authored_fields = fields.authored_members_except([
            (
                "numberOfResourceSlotsPerEntity",
                number_of_resource_slots_per_entity.state(),
            ),
            ("resourceSlotTag", resource_slot_tag.state()),
        ])?;
        Ok(Self {
            source,
            number_of_resource_slots_per_entity,
            resource_slot_tag,
            authored_fields,
        })
    }
}
