// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{Element, SourceElementPosition};

use crate::MaterialError;
use crate::mtl_xml_source::{self, Syntax};

pub(crate) struct PositionedAttribute<'source> {
    pub(crate) ordinal: usize,
    pub(crate) name: &'source str,
    pub(crate) value: &'source str,
}

/// The read-only view of an XML element the material reader needs, over text XML and `CryXml`.
pub(crate) trait XmlElement<'a>: Copy {
    fn syntax(self) -> Result<Syntax<'a>, MaterialError>;

    fn position(self) -> SourceElementPosition;
    fn name(self) -> &'a str;
    fn attribute(self, name: &str) -> Option<&'a str> {
        self.positioned_attribute(name)
            .map(|attribute| attribute.value)
    }
    fn positioned_attribute(self, name: &str) -> Option<PositionedAttribute<'a>>;

    fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)>;

    fn children(self) -> impl Iterator<Item = Self>;
}

impl<'a> XmlElement<'a> for Element<'a> {
    fn syntax(self) -> Result<Syntax<'a>, MaterialError> {
        mtl_xml_source::cryxml(self)
    }

    fn position(self) -> SourceElementPosition {
        SourceElementPosition::CryXml(self.index())
    }

    fn name(self) -> &'a str {
        Element::name(self)
    }

    fn positioned_attribute(self, name: &str) -> Option<PositionedAttribute<'a>> {
        Element::attributes(self)
            .enumerate()
            .find(|(_, attribute)| attribute.name() == name)
            .map(|(ordinal, attribute)| PositionedAttribute {
                ordinal,
                name: attribute.name(),
                value: attribute.value(),
            })
    }

    fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        Element::attributes(self).map(|attribute| (attribute.name(), attribute.value()))
    }

    fn children(self) -> impl Iterator<Item = Self> {
        Element::children(self)
    }
}

impl<'a> XmlElement<'a> for roxmltree::Node<'a, '_> {
    fn syntax(self) -> Result<Syntax<'a>, MaterialError> {
        mtl_xml_source::text(self)
    }

    fn position(self) -> SourceElementPosition {
        SourceElementPosition::Text(self.range())
    }

    fn name(self) -> &'a str {
        self.tag_name().name()
    }

    fn positioned_attribute(self, name: &str) -> Option<PositionedAttribute<'a>> {
        roxmltree::Node::attributes(&self)
            .enumerate()
            .find(|(_, attribute)| attribute.namespace().is_none() && attribute.name() == name)
            .map(|(ordinal, attribute)| PositionedAttribute {
                ordinal,
                name: attribute.name(),
                value: attribute.value(),
            })
    }

    fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        roxmltree::Node::attributes(&self).map(|attribute| (attribute.name(), attribute.value()))
    }

    fn children(self) -> impl Iterator<Item = Self> {
        roxmltree::Node::children(&self).filter(roxmltree::Node::is_element)
    }
}
