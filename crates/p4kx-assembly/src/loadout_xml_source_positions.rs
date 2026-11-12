// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_cryxml::SourceAttributePosition;

use super::{Node, Owner, Scope};

impl Scope<'_, '_> {
    pub(crate) fn attribute_ascii_position(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Option<SourceAttributePosition> {
        match (&self.owner, node) {
            (Owner::Cry(document), Node::Cry(node)) if node.is_from(document) => node
                .source_attributes()
                .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
                .map(|attribute| attribute.position().clone()),
            (Owner::Text(document), Node::Text(node)) if ptr::eq(*document, node.document()) => {
                node.attributes()
                    .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
                    .map(|attribute| {
                        let range = attribute.range();
                        SourceAttributePosition::Text(
                            range.start + self.text_prefix..range.end + self.text_prefix,
                        )
                    })
            }
            _ => None,
        }
    }
}
