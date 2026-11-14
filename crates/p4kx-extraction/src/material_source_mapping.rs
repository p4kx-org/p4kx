// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_model::{
    Attributes, AuthoredAttributeKind, AuthoredContent, Material, MaterialEncoding,
    MaterialEntryDisposition, MaterialLibrary, MaterialSource, MaterialSourceLossKind, RawElement,
};

use crate::material_source_info::{
    MaterialAuthoredInfo, MaterialEncodingInfo, MaterialEntryDispositionInfo,
    MaterialLayerSourceInfo, MaterialLibrarySourceInfo, MaterialSourceEntryInfo,
    MaterialSourceLossInfo, MaterialSourceLossInfoKind, MaterialTextureSourceInfo,
};
use crate::material_xml_info::{
    MaterialAttributeInfo, MaterialAttributeKindInfo, MaterialContentInfo, MaterialElementInfo,
    MaterialElementSyntaxInfo, MaterialParameterInfo,
};

impl<'source> MaterialLibrarySourceInfo<'source> {
    /// Projects one library's authored root without copying its original byte payload.
    #[must_use]
    pub fn of(library: &'source MaterialLibrary) -> Self {
        Self::of_source(&library.authored)
    }

    /// Projects the retained authored root and exact source coverage of one material source.
    #[must_use]
    pub fn of_source(source: &'source MaterialSource) -> Self {
        Self {
            encoding: match source.encoding {
                MaterialEncoding::Text => MaterialEncodingInfo::Text,
                MaterialEncoding::CryXml => MaterialEncodingInfo::CryXml,
                _ => MaterialEncodingInfo::Unsupported,
            },
            root: source.root.as_ref().map(MaterialElementInfo::of),
            losses: source
                .losses
                .iter()
                .map(|loss| MaterialSourceLossInfo {
                    kind: match loss.kind {
                        MaterialSourceLossKind::RepairedEncoding => {
                            MaterialSourceLossInfoKind::RepairedEncoding
                        }
                        MaterialSourceLossKind::Elements => MaterialSourceLossInfoKind::Elements,
                        MaterialSourceLossKind::TextBytes => MaterialSourceLossInfoKind::TextBytes,
                        MaterialSourceLossKind::Depth => MaterialSourceLossInfoKind::Depth,
                        _ => MaterialSourceLossInfoKind::Unsupported,
                    },
                    indices: loss.indices.clone(),
                })
                .collect(),
            entries: source
                .entries
                .iter()
                .map(|entry| MaterialSourceEntryInfo {
                    ordinal: entry.ordinal,
                    element_name: Cow::Borrowed(&entry.element_name),
                    disposition: match entry.disposition {
                        MaterialEntryDisposition::Material => {
                            MaterialEntryDispositionInfo::Material
                        }
                        MaterialEntryDisposition::EngineDefault => {
                            MaterialEntryDispositionInfo::EngineDefault
                        }
                        _ => MaterialEntryDispositionInfo::Unsupported,
                    },
                })
                .collect(),
        }
    }

    /// Returns whether this structured root projection has no loss or unsupported reader value.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.losses.is_empty()
            && self.encoding != MaterialEncodingInfo::Unsupported
            && self
                .root
                .as_ref()
                .is_some_and(MaterialElementInfo::is_complete)
            && self
                .entries
                .iter()
                .all(|entry| entry.disposition != MaterialEntryDispositionInfo::Unsupported)
    }
}

impl<'source> MaterialAuthoredInfo<'source> {
    /// Borrows existing raw material fields without resolving companions or applying palettes.
    #[must_use]
    pub fn of(material: &'source Material) -> Self {
        Self {
            other_attributes: parameters(&material.other_attributes),
            public_params: parameters(&material.public_params),
            layers: material
                .layers
                .iter()
                .map(|layer| MaterialLayerSourceInfo {
                    attributes: parameters(&layer.attributes),
                })
                .collect(),
            elements: material
                .elements
                .iter()
                .map(MaterialElementInfo::of)
                .collect(),
            truncated: material
                .truncated
                .iter()
                .map(|value| Cow::Borrowed(value.as_str()))
                .collect(),
            textures: material
                .textures
                .iter()
                .map(|slot| MaterialTextureSourceInfo {
                    map: Cow::Borrowed(&slot.map),
                    file: Cow::Borrowed(&slot.file),
                    attributes: parameters(&slot.attributes),
                    children: slot.children.iter().map(MaterialElementInfo::of).collect(),
                })
                .collect(),
        }
    }

    /// Returns whether the existing bounded field projections contain no diagnosed loss.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.truncated.is_empty()
            && self.elements.iter().all(MaterialElementInfo::is_complete)
            && self
                .textures
                .iter()
                .all(|slot| slot.children.iter().all(MaterialElementInfo::is_complete))
    }
}

impl<'source> MaterialElementInfo<'source> {
    pub(crate) fn of(element: &'source RawElement) -> Self {
        let syntax = &element.authored;
        Self {
            name: Cow::Borrowed(&element.name),
            attributes: parameters(&element.attributes),
            children: element.children.iter().map(Self::of).collect(),
            syntax: MaterialElementSyntaxInfo {
                ordinal: syntax.ordinal,
                qualified_name: Cow::Borrowed(&syntax.qualified_name),
                namespace: syntax.namespace.as_deref().map(Cow::Borrowed),
                attributes: syntax
                    .attributes
                    .iter()
                    .map(|attribute| MaterialAttributeInfo {
                        name: Cow::Borrowed(&attribute.name),
                        value: Cow::Borrowed(&attribute.value),
                        lexeme: attribute.lexeme.as_deref().map(Cow::Borrowed),
                        kind: match &attribute.kind {
                            AuthoredAttributeKind::Attribute { namespace } => {
                                MaterialAttributeKindInfo::Attribute {
                                    namespace: namespace.as_deref().map(Cow::Borrowed),
                                }
                            }
                            AuthoredAttributeKind::Namespace { prefix } => {
                                MaterialAttributeKindInfo::Namespace {
                                    prefix: prefix.as_deref().map(Cow::Borrowed),
                                }
                            }
                            _ => MaterialAttributeKindInfo::Unsupported,
                        },
                    })
                    .collect(),
                content: syntax
                    .content
                    .iter()
                    .map(|content| match content {
                        AuthoredContent::Text { value, lexeme } => MaterialContentInfo::Text {
                            value: Cow::Borrowed(value),
                            lexeme: Cow::Borrowed(lexeme),
                        },
                        AuthoredContent::Element { ordinal } => {
                            MaterialContentInfo::Element { ordinal: *ordinal }
                        }
                        AuthoredContent::Comment { value } => MaterialContentInfo::Comment {
                            value: Cow::Borrowed(value),
                        },
                        AuthoredContent::ProcessingInstruction { target, value } => {
                            MaterialContentInfo::ProcessingInstruction {
                                target: Cow::Borrowed(target),
                                value: value.as_deref().map(Cow::Borrowed),
                            }
                        }
                        _ => MaterialContentInfo::Unsupported,
                    })
                    .collect(),
                binary_text: syntax.binary_text.as_deref().map(Cow::Borrowed),
            },
        }
    }

    fn is_complete(&self) -> bool {
        self.children.iter().all(Self::is_complete)
            && self
                .syntax
                .attributes
                .iter()
                .all(|attribute| !matches!(attribute.kind, MaterialAttributeKindInfo::Unsupported))
            && self
                .syntax
                .content
                .iter()
                .all(|content| !matches!(content, MaterialContentInfo::Unsupported))
    }
}

fn parameters(attributes: &Attributes) -> Vec<MaterialParameterInfo<'_>> {
    attributes
        .iter()
        .map(|(name, value)| MaterialParameterInfo {
            name: Cow::Borrowed(name),
            value: Cow::Borrowed(value),
        })
        .collect()
}
