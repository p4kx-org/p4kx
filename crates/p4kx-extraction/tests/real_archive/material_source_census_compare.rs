// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    Attributes, AuthoredAttributeKind, AuthoredContent, Material, MaterialEncoding,
    MaterialEntryDisposition, MaterialSource, MaterialSourceLossKind, RawElement,
};
use p4kx_extraction::{
    MaterialAttributeKindInfo, MaterialAuthoredInfo, MaterialContentInfo, MaterialElementInfo,
    MaterialEncodingInfo, MaterialEntryDispositionInfo, MaterialLibrarySourceInfo,
    MaterialParameterInfo, MaterialSourceLossInfoKind,
};

pub(super) fn root(source: &MaterialSource, wire: &MaterialLibrarySourceInfo<'_>) -> bool {
    let root = match (&source.root, &wire.root) {
        (Some(source), Some(wire)) => element(source, wire),
        (None, None) => true,
        _ => false,
    };
    matches!(
        (source.encoding, wire.encoding),
        (MaterialEncoding::Text, MaterialEncodingInfo::Text)
            | (MaterialEncoding::CryXml, MaterialEncodingInfo::CryXml)
    ) && root
        && source.entries.len() == wire.entries.len()
        && source.losses.len() == wire.losses.len()
        && source
            .entries
            .iter()
            .zip(&wire.entries)
            .all(|(source, wire)| {
                source.ordinal == wire.ordinal
                    && source.element_name == wire.element_name
                    && matches!(
                        (source.disposition, wire.disposition),
                        (
                            MaterialEntryDisposition::Material,
                            MaterialEntryDispositionInfo::Material
                        ) | (
                            MaterialEntryDisposition::EngineDefault,
                            MaterialEntryDispositionInfo::EngineDefault
                        )
                    )
            })
        && source
            .losses
            .iter()
            .zip(&wire.losses)
            .all(|(source, wire)| {
                source.indices == wire.indices
                    && matches!(
                        (source.kind, wire.kind),
                        (
                            MaterialSourceLossKind::RepairedEncoding,
                            MaterialSourceLossInfoKind::RepairedEncoding
                        ) | (
                            MaterialSourceLossKind::Elements,
                            MaterialSourceLossInfoKind::Elements
                        ) | (
                            MaterialSourceLossKind::TextBytes,
                            MaterialSourceLossInfoKind::TextBytes
                        ) | (
                            MaterialSourceLossKind::Depth,
                            MaterialSourceLossInfoKind::Depth
                        )
                    )
            })
}

pub(super) fn material(source: &Material, wire: &MaterialAuthoredInfo<'_>) -> bool {
    parameters(&source.other_attributes, &wire.other_attributes)
        && parameters(&source.public_params, &wire.public_params)
        && source.layers.len() == wire.layers.len()
        && source
            .layers
            .iter()
            .zip(&wire.layers)
            .all(|(source, wire)| parameters(&source.attributes, &wire.attributes))
        && source.elements.len() == wire.elements.len()
        && source
            .elements
            .iter()
            .zip(&wire.elements)
            .all(|(source, wire)| element(source, wire))
        && source.truncated.len() == wire.truncated.len()
        && source
            .truncated
            .iter()
            .zip(&wire.truncated)
            .all(|(source, wire)| source == wire.as_ref())
        && source.textures.len() == wire.textures.len()
        && source
            .textures
            .iter()
            .zip(&wire.textures)
            .all(|(source, wire)| {
                source.map == wire.map
                    && source.file == wire.file
                    && parameters(&source.attributes, &wire.attributes)
                    && source.children.len() == wire.children.len()
                    && source
                        .children
                        .iter()
                        .zip(&wire.children)
                        .all(|(source, wire)| element(source, wire))
            })
}

fn parameters(source: &Attributes, wire: &[MaterialParameterInfo<'_>]) -> bool {
    source.len() == wire.len()
        && source
            .iter()
            .zip(wire)
            .all(|((name, value), wire)| name == wire.name.as_ref() && value == wire.value.as_ref())
}

fn element(source: &RawElement, wire: &MaterialElementInfo<'_>) -> bool {
    let syntax = &source.authored;
    let target = &wire.syntax;
    source.name == wire.name
        && parameters(&source.attributes, &wire.attributes)
        && source.children.len() == wire.children.len()
        && source
            .children
            .iter()
            .zip(&wire.children)
            .all(|(source, wire)| element(source, wire))
        && syntax.ordinal == target.ordinal
        && syntax.qualified_name == target.qualified_name
        && syntax.namespace.as_deref() == target.namespace.as_deref()
        && syntax.binary_text.as_deref() == target.binary_text.as_deref()
        && syntax.attributes.len() == target.attributes.len()
        && syntax
            .attributes
            .iter()
            .zip(&target.attributes)
            .all(|(source, wire)| {
                source.name == wire.name
                    && source.value == wire.value
                    && source.lexeme.as_deref() == wire.lexeme.as_deref()
                    && match (&source.kind, &wire.kind) {
                        (
                            AuthoredAttributeKind::Attribute { namespace: source },
                            MaterialAttributeKindInfo::Attribute { namespace: wire },
                        )
                        | (
                            AuthoredAttributeKind::Namespace { prefix: source },
                            MaterialAttributeKindInfo::Namespace { prefix: wire },
                        ) => source.as_deref() == wire.as_deref(),
                        _ => false,
                    }
            })
        && syntax.content.len() == target.content.len()
        && syntax
            .content
            .iter()
            .zip(&target.content)
            .all(|(source, wire)| content(source, wire))
}

fn content(source: &AuthoredContent, wire: &MaterialContentInfo<'_>) -> bool {
    match (source, wire) {
        (
            AuthoredContent::Text {
                value: source,
                lexeme: source_lexeme,
            },
            MaterialContentInfo::Text {
                value: wire,
                lexeme: wire_lexeme,
            },
        ) => source == wire.as_ref() && source_lexeme == wire_lexeme.as_ref(),
        (
            AuthoredContent::Element { ordinal: source },
            MaterialContentInfo::Element { ordinal: wire },
        ) => source == wire,
        (
            AuthoredContent::Comment { value: source },
            MaterialContentInfo::Comment { value: wire },
        ) => source == wire.as_ref(),
        (
            AuthoredContent::ProcessingInstruction {
                target: source,
                value: source_value,
            },
            MaterialContentInfo::ProcessingInstruction {
                target: wire,
                value: wire_value,
            },
        ) => source == wire.as_ref() && source_value.as_deref() == wire_value.as_deref(),
        _ => false,
    }
}
