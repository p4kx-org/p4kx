// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_cryxml::{ByteOrder, Document as CryDocument, Element, Signature, Xml};

#[path = "loadout_xml_source_positions.rs"]
mod source_positions;

use crate::loadout_file_events::Document;
use crate::loadout_xml_attribute::XmlLoadoutAttributeSite;
use crate::resolver_observation::Observation;
use crate::{
    LoadoutClassName, LoadoutClassSelection, LoadoutError, LoadoutFileObservation, LoadoutPath,
    LoadoutPathOperation,
};

/// The actual XML decoding that supplied an authored attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadoutXmlDecoding {
    /// UTF-8 text XML, with the actual leading BOM removal.
    Text {
        /// Whether the parser removed a leading UTF-8 byte order mark.
        utf8_bom: bool,
    },
    /// Binary XML, including the actual string-table repair outcome.
    CryXml {
        /// The actual binary document signature.
        signature: Signature,
        /// The actual binary integer byte order.
        byte_order: ByteOrder,
        /// Whether the decoder repaired invalid UTF-8 string-table bytes.
        repaired_utf8: bool,
    },
}

/// One attribute borrowed from the actual parse of its original file operation.
#[derive(Debug)]
pub struct XmlLoadoutPathSite<'parse> {
    attribute: XmlLoadoutAttributeSite<'parse>,
    operation: LoadoutPathOperation,
}

impl XmlLoadoutPathSite<'_> {
    /// Borrows the genuine original read operation of this parse.
    #[must_use]
    pub const fn observation(&self) -> &LoadoutFileObservation {
        self.attribute.observation()
    }

    /// Returns the original decoder's node index.
    #[must_use]
    pub const fn node_index(&self) -> u32 {
        self.attribute.node_index()
    }

    /// Borrows the actual winning attribute name.
    #[must_use]
    pub const fn attribute_name(&self) -> &str {
        self.attribute.attribute_name()
    }

    /// Borrows the actual decoded attribute value before its path copy.
    #[must_use]
    pub const fn value(&self) -> &str {
        self.attribute.value()
    }

    /// Returns the actual XML decoding operation.
    #[must_use]
    pub const fn decoding(&self) -> LoadoutXmlDecoding {
        self.attribute.decoding()
    }

    /// Returns the actual operation used to copy this path.
    #[must_use]
    pub const fn operation(&self) -> LoadoutPathOperation {
        self.operation
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Node<'scope, 'input> {
    Cry(Element<'scope>),
    Text(roxmltree::Node<'scope, 'input>),
}

impl<'scope> Node<'scope, '_> {
    pub(crate) fn name(self) -> &'scope str {
        match self {
            Self::Cry(node) => node.name(),
            Self::Text(node) => node.tag_name().name(),
        }
    }

    pub(crate) fn try_for_each_attribute(
        self,
        mut visit: impl FnMut(&str, &str) -> Result<(), String>,
    ) -> Result<(), String> {
        match self {
            Self::Cry(node) => {
                for attribute in node.attributes() {
                    visit(attribute.name(), attribute.value())?;
                }
            }
            Self::Text(node) => {
                for attribute in node.attributes() {
                    visit(attribute.name(), attribute.value())?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn try_for_each_child(
        self,
        mut visit: impl FnMut(Self) -> Result<(), String>,
    ) -> Result<(), String> {
        match self {
            Self::Cry(node) => {
                for child in node.children() {
                    visit(Self::Cry(child))?;
                }
            }
            Self::Text(node) => {
                for child in node.children().filter(roxmltree::Node::is_element) {
                    visit(Self::Text(child))?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn attribute(self, name: &str) -> Option<&'scope str> {
        match self {
            Self::Cry(node) => node.attribute(name),
            Self::Text(node) => node.attribute(name),
        }
    }

    pub(crate) fn attribute_ascii(self, name: &str) -> Option<(&'scope str, &'scope str)> {
        match self {
            Self::Cry(node) => node
                .attributes()
                .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
                .map(|attribute| (attribute.name(), attribute.value())),
            Self::Text(node) => node
                .attributes()
                .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
                .map(|attribute| (attribute.name(), attribute.value())),
        }
    }

    pub(crate) fn children(self) -> Vec<Self> {
        match self {
            Self::Cry(node) => node.children().map(Self::Cry).collect(),
            Self::Text(node) => node
                .children()
                .filter(roxmltree::Node::is_element)
                .map(Self::Text)
                .collect(),
        }
    }
}

enum Owner<'scope, 'input> {
    Cry(&'scope CryDocument<'input>),
    Text(&'scope roxmltree::Document<'input>),
}

pub(crate) struct Scope<'scope, 'input> {
    owner: Owner<'scope, 'input>,
    source: Option<&'scope LoadoutFileObservation>,
    observation: Option<&'scope Observation<'scope>>,
    decoding: LoadoutXmlDecoding,
    text_prefix: usize,
}

#[derive(Clone, Copy)]
enum AttributeSelection {
    Path,
    Class,
}

struct Attribute<'node> {
    node_index: u32,
    ordinal: u64,
    name: &'node str,
    value: &'node str,
}

impl Scope<'_, '_> {
    fn attribute<'node>(
        &self,
        node: Node<'node, '_>,
        name: &str,
        selection: AttributeSelection,
    ) -> Result<Option<Attribute<'node>>, String> {
        let attribute = match node {
            Node::Cry(node) => node
                .attributes()
                .enumerate()
                .find(|(_, attribute)| attribute.name() == name)
                .map(|(index, attribute)| (index, attribute.name(), attribute.value())),
            Node::Text(node) => node
                .attributes()
                .enumerate()
                .find(|(_, attribute)| attribute.name() == name)
                .map(|(index, attribute)| (index, attribute.name(), attribute.value())),
        };
        let Some((attribute_index, name, value)) = attribute else {
            return Ok(None);
        };
        let node_index = match (&self.owner, node) {
            (Owner::Cry(document), Node::Cry(node)) if node.is_from(document) => node.index(),
            (Owner::Text(document), Node::Text(node)) if ptr::eq(*document, node.document()) => {
                node.id().get()
            }
            _ => {
                return Err(match selection {
                    AttributeSelection::Path => "the XML path datum belongs to another parse",
                    AttributeSelection::Class => "the XML class datum belongs to another parse",
                }
                .to_owned());
            }
        };
        let attribute_index = u64::try_from(attribute_index)
            .map_err(|_| "the XML attribute ordinal is not representable".to_owned())?;
        Ok(Some(Attribute {
            node_index,
            ordinal: attribute_index,
            name,
            value,
        }))
    }

    pub(crate) fn path(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Result<Option<LoadoutPath>, String> {
        self.path_attribute(node, name, LoadoutPathOperation::Copy)
    }

    pub(crate) fn modification_name(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Result<Option<LoadoutPath>, String> {
        self.path_attribute(node, name, LoadoutPathOperation::VehicleModificationName)
    }

    fn path_attribute(
        &self,
        node: Node<'_, '_>,
        name: &str,
        operation: LoadoutPathOperation,
    ) -> Result<Option<LoadoutPath>, String> {
        let Some(attribute) = self.attribute(node, name, AttributeSelection::Path)? else {
            return Ok(None);
        };
        let (Some(source), Some(observation)) = (self.source, self.observation) else {
            return Ok(Some(LoadoutPath::unbound(attribute.value)));
        };
        let site = XmlLoadoutPathSite {
            attribute: XmlLoadoutAttributeSite {
                observation: source,
                node_index: attribute.node_index,
                attribute_index: attribute.ordinal,
                attribute_name: attribute.name,
                value: attribute.value,
                decoding: self.decoding,
            },
            operation,
        };
        LoadoutPath::xml(&site, |selection| observation.xml_path(selection, &site)).map(Some)
    }

    pub(crate) fn class_name(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Result<Option<LoadoutClassName>, String> {
        let Some(attribute) = self.attribute(node, name, AttributeSelection::Class)? else {
            return Ok(None);
        };
        if attribute.value.is_empty() {
            return Ok(None);
        }
        let (Some(source), Some(observation)) = (self.source, self.observation) else {
            return Ok(Some(LoadoutClassName::unbound(attribute.value)));
        };
        let site = XmlLoadoutAttributeSite {
            observation: source,
            node_index: attribute.node_index,
            attribute_index: attribute.ordinal,
            attribute_name: attribute.name,
            value: attribute.value,
            decoding: self.decoding,
        };
        let selection = LoadoutClassSelection::new();
        if observation.xml_class(&selection, &site).is_break() {
            return Err(LoadoutError::ObservationStopped.to_string());
        }
        Ok(Some(LoadoutClassName::captured(
            site.value(),
            Some(selection),
        )))
    }
}

pub(crate) fn parse<T>(
    document: &Document,
    observation: Option<&Observation<'_>>,
    read: impl FnOnce(Node<'_, '_>, &Scope<'_, '_>) -> Result<T, String>,
) -> Result<T, String> {
    parse_bytes(document, document.observation(), observation, read)
}

pub(crate) fn parse_unbound<T>(
    bytes: &[u8],
    read: impl FnOnce(Node<'_, '_>, &Scope<'_, '_>) -> Result<T, String>,
) -> Result<T, String> {
    parse_bytes(bytes, None, None, read)
}

fn parse_bytes<T>(
    bytes: &[u8],
    source: Option<&LoadoutFileObservation>,
    observation: Option<&Observation<'_>>,
    read: impl FnOnce(Node<'_, '_>, &Scope<'_, '_>) -> Result<T, String>,
) -> Result<T, String> {
    match p4kx_cryxml::read(bytes).map_err(|error| error.to_string())? {
        Xml::CryXml(document) => {
            let scope = Scope {
                owner: Owner::Cry(&document),
                text_prefix: 0,
                source,
                observation,
                decoding: LoadoutXmlDecoding::CryXml {
                    signature: document.signature(),
                    byte_order: document.byte_order(),
                    repaired_utf8: document.is_lossy(),
                },
            };
            read(Node::Cry(document.root()), &scope)
        }
        Xml::Text(text) => {
            let original = std::str::from_utf8(text).map_err(|error| error.to_string())?;
            let text = original.trim_start_matches('\u{feff}');
            let text_prefix = original.len() - text.len();
            let decoding = LoadoutXmlDecoding::Text {
                utf8_bom: text_prefix != 0,
            };
            p4kx_cryxml::check_text_depth(text).map_err(|error| error.to_string())?;
            let document = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
            let scope = Scope {
                owner: Owner::Text(&document),
                text_prefix,
                source,
                observation,
                decoding,
            };
            read(Node::Text(document.root_element()), &scope)
        }
    }
}
#[cfg(test)]
#[path = "loadout_xml_tests.rs"]
mod tests;
