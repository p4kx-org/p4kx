// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_size;
use crate::mtl_source::{
    AuthoredAttribute, AuthoredAttributeKind, AuthoredContent, ElementAuthored, MaterialSource,
    MaterialSourceEntry, MaterialSourceLoss,
};

pub(crate) fn library(source: &MaterialSource) -> usize {
    size_of::<MaterialSource>()
        + source.root.as_ref().map_or(0, mtl_size::element)
        + source.losses.capacity() * size_of::<MaterialSourceLoss>()
        + source
            .losses
            .iter()
            .map(|loss| loss.indices.capacity() * size_of::<usize>())
            .sum::<usize>()
        + source.entries.capacity() * size_of::<MaterialSourceEntry>()
        + source
            .entries
            .iter()
            .map(|entry| entry.element_name.len())
            .sum::<usize>()
}

pub(crate) fn element(authored: &ElementAuthored) -> usize {
    authored.qualified_name.len()
        + authored.namespace.as_ref().map_or(0, String::len)
        + authored.binary_text.as_ref().map_or(0, String::len)
        + authored.attributes.capacity() * size_of::<AuthoredAttribute>()
        + authored.attributes.iter().map(attribute).sum::<usize>()
        + authored.content.capacity() * size_of::<AuthoredContent>()
        + authored.content.iter().map(content).sum::<usize>()
}

fn attribute(attribute: &AuthoredAttribute) -> usize {
    let meaning = match &attribute.kind {
        AuthoredAttributeKind::Attribute { namespace } => namespace.as_ref().map_or(0, String::len),
        AuthoredAttributeKind::Namespace { prefix } => prefix.as_ref().map_or(0, String::len),
    };
    attribute.name.len()
        + attribute.value.len()
        + attribute.lexeme.as_ref().map_or(0, String::len)
        + meaning
}

fn content(content: &AuthoredContent) -> usize {
    match content {
        AuthoredContent::Text { value, lexeme } => value.len() + lexeme.len(),
        AuthoredContent::Element { .. } => 0,
        AuthoredContent::Comment { value } => value.len(),
        AuthoredContent::ProcessingInstruction { target, value } => {
            target.len() + value.as_ref().map_or(0, String::len)
        }
    }
}
