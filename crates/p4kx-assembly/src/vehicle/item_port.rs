// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{PortVisibility, ValueOrigin};

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
    pub(crate) min_size: Option<u32>,
    pub(crate) max_size: Option<u32>,
    pub(crate) flags: String,
    pub(crate) id: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) port_tags: String,
    pub(crate) required_tags: String,
    pub(crate) types: Vec<String>,
    pub(crate) item_types: Vec<VehicleItemType>,
    pub(crate) light_attachments: Vec<super::VehicleLightAttachment>,
    pub(crate) origins: Vec<(String, ValueOrigin)>,
}

impl VehicleItemPort {
    /// Returns the tags the port offers, as stored: words separated by white space.
    #[must_use]
    pub fn port_tags(&self) -> &str {
        &self.port_tags
    }

    /// Returns the tags an item must carry to fit the port, as stored: words separated by white
    /// space.
    #[must_use]
    pub fn required_tags(&self) -> &str {
        &self.required_tags
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
        self.display_name.as_deref()
    }

    /// Returns the smallest item size the port takes, or `None` when the XML gives none.
    #[must_use]
    pub const fn min_size(&self) -> Option<u32> {
        self.min_size
    }

    /// Returns the largest item size the port takes, or `None` when the XML gives none.
    #[must_use]
    pub const fn max_size(&self) -> Option<u32> {
        self.max_size
    }

    /// Returns the item port's own id, which Modifications address, if it has one.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Returns the flags as stored, e.g. `invisible $uneditable`.
    #[must_use]
    pub fn flags(&self) -> &str {
        &self.flags
    }

    /// Returns whether the flags contain `flag`, ignoring ASCII case and a leading `$`.
    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        flags_contain(&self.flags, flag)
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
        self.origins
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map_or(ValueOrigin::Base, |(_, origin)| *origin)
    }
}
