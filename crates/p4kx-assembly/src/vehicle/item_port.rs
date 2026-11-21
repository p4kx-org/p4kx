// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;

use super::{PortVisibility, ValueOrigin, VehicleAttributeEvidence};
use crate::markup::AttributeOrigins;

pub(crate) const PORT_OBSERVED_ATTRIBUTES: [&str; 5] = [
    "minSize",
    "maxSize",
    "portTags",
    "requiredTags",
    "display_name",
];

/// Returns whether `flags`, words separated by white space, contain `flag`, ignoring ASCII case
/// and a leading `$`.
pub(crate) fn flags_contain(flags: &str, flag: &str) -> bool {
    flags
        .split_ascii_whitespace()
        .any(|token| token.trim_start_matches('$').eq_ignore_ascii_case(flag))
}

/// One `Types/Type` element of an item port: an item type and the sub-types of it the port takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleItemType {
    pub(crate) item_type: String,
    pub(crate) subtypes: Vec<String>,
}

impl VehicleItemType {
    /// Returns the item type, e.g. `WeaponGun`.
    #[must_use]
    pub fn item_type(&self) -> &str {
        &self.item_type
    }

    /// Returns the sub-types the port takes of the item type, in file order; empty when the
    /// element names none, which takes every sub-type.
    #[must_use]
    pub fn subtypes(&self) -> &[String] {
        &self.subtypes
    }
}

/// The item port of a part of the vehicle implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleItemPort {
    pub(crate) min_size: SourceValue<u32>,
    pub(crate) max_size: SourceValue<u32>,
    pub(crate) flags: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) port_tags: Option<String>,
    pub(crate) required_tags: Option<String>,
    pub(crate) types: Vec<String>,
    pub(crate) item_types: Vec<VehicleItemType>,
    pub(crate) light_attachments: Vec<super::VehicleLightAttachment>,
    pub(crate) origins: AttributeOrigins,
}

impl VehicleItemPort {
    /// Returns the tags the port offers, as stored: words separated by white space.
    #[must_use]
    pub fn port_tags(&self) -> &str {
        self.port_tags.as_deref().unwrap_or_default()
    }

    /// Returns the tags an item must carry to fit the port, as stored: words separated by white
    /// space.
    #[must_use]
    pub fn required_tags(&self) -> &str {
        self.required_tags.as_deref().unwrap_or_default()
    }

    /// Borrows the applied port tags and their original attribute evidence.
    #[must_use]
    pub fn port_tags_source(&self) -> (Option<&str>, Option<&VehicleAttributeEvidence>) {
        (
            self.port_tags.as_deref(),
            self.origins
                .evidence("portTags")
                .map(std::sync::Arc::as_ref),
        )
    }

    /// Borrows the required attachment tags and their original attribute evidence.
    #[must_use]
    pub fn required_tags_source(&self) -> (Option<&str>, Option<&VehicleAttributeEvidence>) {
        (
            self.required_tags.as_deref(),
            self.origins
                .evidence("requiredTags")
                .map(std::sync::Arc::as_ref),
        )
    }

    /// Returns the item types the port takes, the `type` of each `Types/Type` element.
    #[must_use]
    pub fn types(&self) -> &[String] {
        &self.types
    }

    /// Returns the `Types/Type` elements of the port with their sub-types, in file order.
    #[must_use]
    pub fn item_types(&self) -> &[VehicleItemType] {
        &self.item_types
    }

    /// Returns the display name the port gives itself, a localisation key as stored, or `None`
    /// when the XML gives none.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref().filter(|name| !name.is_empty())
    }

    /// Borrows the original display name, including empty text, and its effective attribute evidence.
    #[must_use]
    pub fn display_name_source(&self) -> (Option<&str>, Option<&VehicleAttributeEvidence>) {
        let evidence = self
            .origins
            .evidence("display_name")
            .map(std::sync::Arc::as_ref);
        let authored = self
            .display_name
            .as_deref()
            .filter(|_| !evidence.is_some_and(|site| site.original().checked_absence()));
        (authored, evidence)
    }

    /// Returns the smallest item size the port takes, or `None` when the XML gives none.
    #[must_use]
    pub const fn min_size(&self) -> Option<u32> {
        match self.min_size {
            SourceValue::Present(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the largest item size the port takes, or `None` when the XML gives none.
    #[must_use]
    pub const fn max_size(&self) -> Option<u32> {
        match self.max_size {
            SourceValue::Present(value) => Some(value),
            _ => None,
        }
    }

    /// Borrows the minimum size and the effective attribute that supplied or omitted it.
    #[must_use]
    pub fn min_size_source(&self) -> (&SourceValue<u32>, Option<&VehicleAttributeEvidence>) {
        (
            &self.min_size,
            self.origins.evidence("minSize").map(std::sync::Arc::as_ref),
        )
    }

    /// Borrows the maximum size and the effective attribute that supplied or omitted it.
    #[must_use]
    pub fn max_size_source(&self) -> (&SourceValue<u32>, Option<&VehicleAttributeEvidence>) {
        (
            &self.max_size,
            self.origins.evidence("maxSize").map(std::sync::Arc::as_ref),
        )
    }

    /// Returns the item port's own id, which Modifications address, if it has one.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Returns the flags as stored, e.g. `invisible $uneditable`.
    #[must_use]
    pub fn flags(&self) -> &str {
        self.flags.as_deref().unwrap_or_default()
    }

    /// Returns whether the flags contain `flag`, ignoring ASCII case and a leading `$`.
    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        flags_contain(self.flags(), flag)
    }

    /// Returns [`PortVisibility::Invisible`] when the port carries the flag `invisible`.
    #[must_use]
    pub fn visibility(&self) -> PortVisibility {
        if self.has_flag("invisible") {
            PortVisibility::Invisible
        } else {
            PortVisibility::Visible
        }
    }

    /// Returns where the attribute `name` of the item port got its value, ignoring ASCII case.
    #[must_use]
    pub fn origin_of(&self, name: &str) -> ValueOrigin {
        self.origins.origin_of(name)
    }

    /// Returns the original attribute evidence supplying the effective flags.
    #[must_use]
    pub fn flags_evidence(&self) -> Option<&VehicleAttributeEvidence> {
        self.origins.evidence("flags").map(std::sync::Arc::as_ref)
    }

    /// Borrows the applied flags and their original attribute evidence.
    #[must_use]
    pub fn flags_source(&self) -> (Option<&str>, Option<&VehicleAttributeEvidence>) {
        (self.flags.as_deref(), self.flags_evidence())
    }
}
