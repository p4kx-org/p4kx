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
pub struct PortRestrictions<'source> {
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
    /// Borrows the original ordered item type restrictions.
    #[must_use]
    pub const fn types(&self) -> &SourcedValue<Vec<SourcedValue<PortItemType<'source>>>> {
        &self.types
    }

    /// Borrows the minimum size or its retained original shape.
    #[must_use]
    pub const fn min_size(&self) -> &SourcedValue<AuthoredRestriction<'source, ResourceNumber>> {
        &self.min_size
    }

    /// Borrows the maximum size or its retained original shape.
    #[must_use]
    pub const fn max_size(&self) -> &SourcedValue<AuthoredRestriction<'source, ResourceNumber>> {
        &self.max_size
    }

    /// Borrows the original Tag reference inventory.
    #[must_use]
    pub const fn tag_references(&self) -> &SourcedValue<SourceArenaOutput<'source>> {
        &self.tag_references
    }

    /// Borrows the original port rule inventory.
    #[must_use]
    pub const fn item_port_rules(&self) -> &SourcedValue<SourceArenaOutput<'source>> {
        &self.item_port_rules
    }

    /// Borrows the original port flags.
    #[must_use]
    pub const fn flags(&self) -> &SourcedValue<&'source str> {
        &self.flags
    }

    /// Borrows the port's authored tags.
    #[must_use]
    pub const fn port_tags(&self) -> &SourcedValue<&'source str> {
        &self.port_tags
    }

    /// Borrows the tags an attachment must satisfy.
    #[must_use]
    pub const fn required_port_tags(&self) -> &SourcedValue<&'source str> {
        &self.required_port_tags
    }

    /// Borrows the original resource-link setting.
    #[must_use]
    pub const fn resource_link_to_parent(&self) -> &SourcedValue<bool> {
        &self.resource_link_to_parent
    }

    #[cfg(test)]
    pub(super) fn fixture(sources: &p4kx_extraction::SourceUses) -> Self {
        use p4kx_extraction::SourceValue;

        Self {
            types: sources.value(SourceValue::Absent, &[], true),
            min_size: sources.value(SourceValue::Absent, &[], true),
            max_size: sources.value(SourceValue::Absent, &[], true),
            flags: sources.value(SourceValue::Absent, &[], true),
            port_tags: sources.value(SourceValue::Absent, &[], true),
            required_port_tags: sources.value(SourceValue::Absent, &[], true),
            tag_references: sources.value(SourceValue::Absent, &[], true),
            item_port_rules: sources.value(SourceValue::Absent, &[], true),
            resource_link_to_parent: sources.value(SourceValue::Absent, &[], true),
        }
    }

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
pub struct PortItemType<'source> {
    r#type: SourcedValue<AuthoredRestriction<'source, &'source str>>,
    sub_types: OriginalMembers<'source>,
}

impl<'source> PortItemType<'source> {
    /// Borrows the item type or its retained original shape.
    #[must_use]
    pub const fn item_type(&self) -> &SourcedValue<AuthoredRestriction<'source, &'source str>> {
        &self.r#type
    }

    /// Borrows the original subtype inventory.
    #[must_use]
    pub const fn sub_types(&self) -> &SourcedValue<SourceArenaOutput<'source>> {
        &self.sub_types
    }

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
