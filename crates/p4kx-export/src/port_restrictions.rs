// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::{AuthoredRestriction, Observed};
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

type OriginalMembers<'source> = SourcedValue<SourceArenaOutput<'source>>;

/// The physical port restrictions and network-to-parent flag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct PortRestrictions<'source> {
    types: SourcedValue<Vec<SourcedValue<PortItemType<'source>>>>,
    min_size: SourcedValue<AuthoredRestriction<'source, ResourceNumber>>,
    max_size: SourcedValue<AuthoredRestriction<'source, ResourceNumber>>,
    flags: SourcedValue<&'source str>,
    port_tags: SourcedValue<&'source str>,
    required_port_tags: SourcedValue<&'source str>,
    tag_references: OriginalMembers<'source>,
    item_port_rules: OriginalMembers<'source>,
    resource_link_to_parent: SourcedValue<bool>,
}

impl<'source> PortRestrictions<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            types: source.class_array_to(
                "Types",
                "SItemPortDefTypes",
                notes,
                PortItemType::read,
            )?,
            min_size: source.restriction_number("MinSize", notes)?,
            max_size: source.restriction_number("MaxSize", notes)?,
            flags: source.borrowed_string("Flags", notes)?,
            port_tags: source.borrowed_string("PortTags", notes)?,
            required_port_tags: source.borrowed_string("RequiredPortTags", notes)?,
            tag_references: source.authored_member("Tags", notes)?,
            item_port_rules: source.authored_member("itemPortRules", notes)?,
            resource_link_to_parent: source.boolean("resourceLinkToParent", notes)?,
        })
    }
}

/// One declared accepted item type and its exact authored subtype array.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PortItemType<'source> {
    r#type: SourcedValue<AuthoredRestriction<'source, &'source str>>,
    sub_types: OriginalMembers<'source>,
}

impl<'source> PortItemType<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            r#type: source.restriction_label("Type", notes)?,
            sub_types: source.authored_member("SubTypes", notes)?,
        })
    }
}
