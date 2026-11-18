// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::SourceLocation;

use super::{Node, Owner, Scope};
use crate::resolver_observation::Observation;
use crate::{LoadoutFileObservation, LoadoutXmlDecoding, VehicleXmlSite};

#[derive(Clone, Copy)]
pub(crate) enum MissingAttribute {
    Omit,
    Element,
}

pub(super) struct OriginalAttribute<'node> {
    pub(super) ordinal: usize,
    pub(super) name: &'node str,
    pub(super) value: &'node str,
    pub(super) position: SourceAttributePosition,
}

impl Scope<'_, '_> {
    pub(super) fn original_node_index(&self, node: Node<'_, '_>) -> Option<u32> {
        match (&self.owner, node) {
            (Owner::Cry(document), Node::Cry(node)) if node.is_from(document) => Some(node.index()),
            (Owner::Text(document), Node::Text(node)) if ptr::eq(*document, node.document()) => {
                Some(node.id().get())
            }
            _ => None,
        }
    }

    pub(super) fn original_attribute<'node>(
        &self,
        node: Node<'node, '_>,
        mut matches: impl FnMut(&str) -> bool,
    ) -> Option<OriginalAttribute<'node>> {
        self.original_attribute_where(node, |_, name| matches(name))
    }

    fn original_attribute_where<'node>(
        &self,
        node: Node<'node, '_>,
        mut matches: impl FnMut(usize, &str) -> bool,
    ) -> Option<OriginalAttribute<'node>> {
        match node {
            Node::Cry(node) => node
                .source_attributes()
                .enumerate()
                .find(|(ordinal, attribute)| matches(*ordinal, attribute.name()))
                .map(|(ordinal, attribute)| OriginalAttribute {
                    ordinal,
                    name: attribute.name(),
                    value: attribute.value(),
                    position: attribute.position().clone(),
                }),
            Node::Text(node) => node
                .attributes()
                .enumerate()
                .find(|(ordinal, attribute)| matches(*ordinal, attribute.name()))
                .map(|(ordinal, attribute)| {
                    let range = attribute.range();
                    OriginalAttribute {
                        ordinal,
                        name: attribute.name(),
                        value: attribute.value(),
                        position: SourceAttributePosition::Text(
                            range.start + self.text_prefix..range.end + self.text_prefix,
                        ),
                    }
                }),
        }
    }

    fn original_element_position(&self, node: Node<'_, '_>) -> Option<SourceElementPosition> {
        let index = self.original_node_index(node)?;
        Some(match node {
            Node::Cry(_) => SourceElementPosition::CryXml(index),
            Node::Text(node) => {
                let range = node.range();
                SourceElementPosition::Text(
                    range.start + self.text_prefix..range.end + self.text_prefix,
                )
            }
        })
    }

    pub(crate) fn vehicle_element_site(&self, node: Node<'_, '_>) -> Option<VehicleXmlSite> {
        let source = self.vehicle_source()?;
        let position = self.original_element_position(node)?;
        Some(self.vehicle_site_at_location(source, SourceLocation::xml_element(&position, "$node")))
    }

    pub(crate) fn vehicle_site(
        &self,
        node: Node<'_, '_>,
        name: &str,
        missing: MissingAttribute,
    ) -> Option<VehicleXmlSite> {
        let source = self.vehicle_source()?;
        let element = self.original_element_position(node)?;
        let attribute = self.original_attribute(node, |actual| actual.eq_ignore_ascii_case(name));
        let location = match attribute {
            Some(attribute) => SourceLocation::xml_attribute(&attribute.position, attribute.name),
            None if matches!(missing, MissingAttribute::Element) => {
                SourceLocation::xml_element(&element, name)
            }
            None => return None,
        };
        Some(self.vehicle_site_at_location(source, location))
    }

    pub(crate) fn vehicle_site_at(
        &self,
        node: Node<'_, '_>,
        ordinal: usize,
        name: &str,
    ) -> Option<VehicleXmlSite> {
        let source = self.vehicle_source()?;
        self.original_node_index(node)?;
        let attribute = self
            .original_attribute_where(node, |index, actual| index == ordinal && actual == name)?;
        let location = SourceLocation::xml_attribute(&attribute.position, attribute.name);
        Some(self.vehicle_site_at_location(source, location))
    }

    pub(crate) fn observes_vehicle_totals(&self) -> bool {
        self.observation
            .is_some_and(Observation::observes_vehicle_totals)
    }

    fn vehicle_source(&self) -> Option<&LoadoutFileObservation> {
        self.source.filter(|_| {
            self.observation.is_some_and(|observer| {
                observer.observes_vehicle_visibility() || observer.observes_vehicle_totals()
            })
        })
    }

    fn vehicle_site_at_location(
        &self,
        source: &LoadoutFileObservation,
        location: Option<SourceLocation>,
    ) -> VehicleXmlSite {
        let lossless = !matches!(
            self.decoding,
            LoadoutXmlDecoding::CryXml {
                repaired_utf8: true,
                ..
            }
        );
        VehicleXmlSite::new(source.clone(), location.filter(|_| lossless))
    }

    pub(crate) fn attribute_ascii_position(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Option<SourceAttributePosition> {
        self.original_node_index(node)?;
        self.original_attribute(node, |actual| actual.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.position)
    }
}

pub(crate) struct AttributeNamespaces {
    pub(crate) text_bytes: &'static str,
    pub(crate) cryxml_attribute: &'static str,
}

impl AttributeNamespaces {
    pub(crate) fn location(
        self,
        position: &SourceAttributePosition,
        field: &str,
    ) -> Option<SourceLocation> {
        let (namespace, indices) = match position {
            SourceAttributePosition::Text(range) => (
                self.text_bytes,
                vec![
                    u64::try_from(range.start).ok()?,
                    u64::try_from(range.end).ok()?,
                ],
            ),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                self.cryxml_attribute,
                vec![u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => return None,
        };
        Some(SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        })
    }
}
