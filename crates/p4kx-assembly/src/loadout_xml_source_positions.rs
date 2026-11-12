// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

use super::{Node, Owner, Scope};

pub(super) struct OriginalAttribute<'node> {
    pub(super) ordinal: usize,
    pub(super) name: &'node str,
    pub(super) value: &'node str,
    position: SourceAttributePosition,
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
        match node {
            Node::Cry(node) => node
                .source_attributes()
                .enumerate()
                .find(|(_, attribute)| matches(attribute.name()))
                .map(|(ordinal, attribute)| OriginalAttribute {
                    ordinal,
                    name: attribute.name(),
                    value: attribute.value(),
                    position: attribute.position().clone(),
                }),
            Node::Text(node) => node
                .attributes()
                .enumerate()
                .find(|(_, attribute)| matches(attribute.name()))
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
