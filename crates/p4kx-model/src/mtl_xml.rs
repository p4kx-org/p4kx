// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::Element;

use crate::MaterialError;
use crate::mtl_xml_source::{self, Syntax};

/// The read-only view of an XML element the material reader needs, over text XML and `CryXml`.
pub(crate) trait XmlElement<'a>: Copy {
    fn syntax(self) -> Result<Syntax<'a>, MaterialError>;

    fn name(self) -> &'a str;

    fn attribute(self, name: &str) -> Option<&'a str>;

    fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)>;

    fn children(self) -> impl Iterator<Item = Self>;
}

impl<'a> XmlElement<'a> for Element<'a> {
    fn syntax(self) -> Result<Syntax<'a>, MaterialError> {
        mtl_xml_source::cryxml(self)
    }

    fn name(self) -> &'a str {
        Element::name(self)
    }

    fn attribute(self, name: &str) -> Option<&'a str> {
        Element::attribute(self, name)
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

    fn name(self) -> &'a str {
        self.tag_name().name()
    }

    fn attribute(self, name: &str) -> Option<&'a str> {
        roxmltree::Node::attribute(&self, name)
    }

    fn attributes(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        roxmltree::Node::attributes(&self).map(|attribute| (attribute.name(), attribute.value()))
    }

    fn children(self) -> impl Iterator<Item = Self> {
        roxmltree::Node::children(&self).filter(roxmltree::Node::is_element)
    }
}
