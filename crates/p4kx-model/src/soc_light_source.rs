// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElement;

use crate::LightState;
use crate::soc_source_node::{SocSourceNode, SocSourceValue, named};

/// One original light component and its position in an Entity document.
#[derive(Debug, Clone, Copy)]
pub struct SocLightSource<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocLightSource<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(entity, "PropertiesDataCore").flat_map(|properties| {
            SocSourceNode::children(properties.element(), "EntityComponentLight")
                .map(|node| Self { node })
        })
    }

    /// Returns the original light component and its attributes and nested fields.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Reads the unique authored light type.
    #[must_use]
    pub fn light_type(self) -> SocSourceValue<&'document str> {
        self.node
            .read("lightType", |value| (!value.is_empty()).then_some(value))
    }

    /// Reads the authored temperature switch, retaining explicit false.
    #[must_use]
    pub fn use_temperature(self) -> SocSourceValue<bool> {
        self.node.read("useTemperature", boolean)
    }

    /// Visits all authored modes in original order, including repeated and empty modes.
    pub fn modes(self) -> impl Iterator<Item = SocLightMode<'document>> {
        SocSourceNode::direct_children(self.element()).filter_map(|node| {
            let kind = LightState::from_element(node.element())?;
            Some(SocLightMode { kind, node })
        })
    }
}

/// One physical authored light mode.
#[derive(Debug, Clone, Copy)]
pub struct SocLightMode<'document> {
    kind: LightState,
    node: SocSourceNode<'document>,
}

impl<'document> SocLightMode<'document> {
    /// Returns the mode identified by its original element name.
    #[must_use]
    pub const fn kind(self) -> LightState {
        self.kind
    }

    /// Returns the original mode element and its exact coordinates.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Reads the finite authored intensity in the engine's unit.
    #[must_use]
    pub fn intensity(self) -> SocSourceValue<f32> {
        self.node.read("intensity", finite_single)
    }

    /// Reads the finite authored colour temperature in kelvin.
    #[must_use]
    pub fn temperature(self) -> SocSourceValue<f32> {
        self.node.read("temperature", finite_single)
    }

    /// Reads the authored light style index.
    #[must_use]
    pub fn light_style(self) -> SocSourceValue<u32> {
        self.node.read("lightStyle", |value| value.parse().ok())
    }

    /// Visits the original colour nodes, retaining repeated occurrences.
    pub fn colours(self) -> impl Iterator<Item = SourceElement<'document>> {
        SocSourceNode::children(self.element(), "color").map(SocSourceNode::element)
    }
}

/// One original light group owned by an Entity.
#[derive(Debug, Clone, Copy)]
pub struct SocLightGroupSource<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocLightGroupSource<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::direct_children(entity).flat_map(|node| {
            let direct = named(node.element(), "EntityComponentLightGroup").then_some(node);
            let properties = named(node.element(), "PropertiesDataCore").then_some(node);
            direct
                .into_iter()
                .chain(properties.into_iter().flat_map(|properties| {
                    SocSourceNode::children(properties.element(), "EntityComponentLightGroup")
                }))
                .map(|node| Self { node })
        })
    }

    /// Returns the group's original element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits original baked-light sections, including explicit empty sections.
    pub fn baked_sections(self) -> impl Iterator<Item = SourceElement<'document>> {
        SocSourceNode::children(self.element(), "BakedInLights").map(SocSourceNode::element)
    }

    /// Visits authored baked-light owners in section and element order.
    pub fn lights(self) -> impl Iterator<Item = SocBakedLightSource<'document>> {
        self.baked_sections().flat_map(|section| {
            SocSourceNode::children(section, "Light")
                .map(move |node| SocBakedLightSource { section, node })
        })
    }
}

/// One authored baked-light owner in a light group.
#[derive(Debug, Clone, Copy)]
pub struct SocBakedLightSource<'document> {
    section: SourceElement<'document>,
    node: SocSourceNode<'document>,
}

impl<'document> SocBakedLightSource<'document> {
    /// Returns the original baked-light section.
    #[must_use]
    pub const fn section(self) -> SourceElement<'document> {
        self.section
    }

    /// Returns the original baked-light owner.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits the local frame declarations in original order.
    pub fn relative_frames(self) -> impl Iterator<Item = SourceElement<'document>> {
        SocSourceNode::children(self.element(), "RelativeXForm").map(SocSourceNode::element)
    }

    /// Visits all light components of this baked-light owner.
    pub fn components(self) -> impl Iterator<Item = SocLightSource<'document>> {
        SocSourceNode::children(self.element(), "EntityComponentLight")
            .map(|node| SocLightSource { node })
    }
}

fn finite_single(value: &str) -> Option<f32> {
    let value: f32 = value.parse().ok()?;
    value.is_finite().then_some(value)
}

fn boolean(value: &str) -> Option<bool> {
    if value == "1" || value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value == "0" || value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}
