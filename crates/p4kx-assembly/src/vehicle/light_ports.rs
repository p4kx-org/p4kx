// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::SocSourceValue;

use super::{ValueOrigin, VehicleImplementation, VehicleItemPort, VehiclePart};
use crate::markup::Element;

const MULTI_LIGHT: &str = "MultiLight";

/// One authored `MultiLight` attachment-point occurrence after vehicle modifications.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VehicleLightAttachment {
    /// Ordinal of the `AttachmentPoints` section within the item port.
    pub section: usize,
    /// Ordinal of the `AttachmentPoint` within that section.
    pub ordinal: usize,
    name: Field,
    bone: Field,
    bone_position: Option<p4kx_cryxml::SourceAttributePosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field(Vec<(String, ValueOrigin)>);

impl Field {
    fn read(element: &Element, name: &str) -> Self {
        Self(
            element
                .attributes
                .iter()
                .filter(|(key, _)| key == name)
                .map(|(_, value)| (value.clone(), element.origin_of(name)))
                .collect(),
        )
    }

    fn value(&self) -> SocSourceValue<&str> {
        match self.0.as_slice() {
            [] => SocSourceValue::Absent,
            [(value, _)] if !value.is_empty() => SocSourceValue::Present(value),
            _ => SocSourceValue::Invalid,
        }
    }
}

impl VehicleLightAttachment {
    /// Returns the unique nonempty authored attachment point name.
    #[must_use]
    pub fn name(&self) -> SocSourceValue<&str> {
        self.name.value()
    }

    /// Returns the unique nonempty authored helper or bone name.
    #[must_use]
    pub fn bone(&self) -> SocSourceValue<&str> {
        self.bone.value()
    }

    /// Visits original name attribute values with their file or modification origins.
    pub fn name_values(&self) -> impl Iterator<Item = (&str, ValueOrigin)> {
        self.name
            .0
            .iter()
            .map(|(value, origin)| (value.as_str(), *origin))
    }

    /// Returns the winning bone attribute's original physical coordinate.
    #[must_use]
    pub const fn bone_position(&self) -> Option<&p4kx_cryxml::SourceAttributePosition> {
        self.bone_position.as_ref()
    }

    /// Visits original bone attribute values with their file or modification origins.
    pub fn bone_values(&self) -> impl Iterator<Item = (&str, ValueOrigin)> {
        self.bone
            .0
            .iter()
            .map(|(value, origin)| (value.as_str(), *origin))
    }
}

/// One effective `MultiLight` item port on the selected vehicle implementation.
#[derive(Debug, Clone, Copy)]
pub struct VehicleLightPort<'implementation> {
    part: &'implementation VehiclePart,
    port: &'implementation VehicleItemPort,
}

impl<'implementation> VehicleLightPort<'implementation> {
    /// Returns the effective part that owns the original item port.
    #[must_use]
    pub const fn part(self) -> &'implementation VehiclePart {
        self.part
    }

    /// Returns attachment points in original effective port order.
    #[must_use]
    pub fn attachments(self) -> &'implementation [VehicleLightAttachment] {
        &self.port.light_attachments
    }
}

impl VehicleImplementation {
    /// Visits all effective `MultiLight` item ports in part order.
    pub fn light_ports(&self) -> impl Iterator<Item = VehicleLightPort<'_>> {
        self.item_ports().filter_map(|part| {
            let port = part.item_port()?;
            port.types()
                .iter()
                .any(|kind| kind == MULTI_LIGHT)
                .then_some(VehicleLightPort { part, port })
        })
    }
}

pub(super) fn attachments(port: &Element) -> Vec<VehicleLightAttachment> {
    port.children
        .iter()
        .enumerate()
        .filter(|(_, element)| element.name == "AttachmentPoints")
        .flat_map(|(section, element)| {
            element
                .children
                .iter()
                .enumerate()
                .filter(|(_, element)| element.name == "AttachmentPoint")
                .map(move |(ordinal, element)| VehicleLightAttachment {
                    section,
                    ordinal,
                    name: Field::read(element, "name"),
                    bone: Field::read(element, "bone"),
                    bone_position: element.light_bone.clone(),
                })
        })
        .collect()
}
