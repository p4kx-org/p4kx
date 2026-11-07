// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::material_source_info::{
    MaterialAuthoredInfo, MaterialLayerSourceInfo, MaterialLibrarySourceInfo,
    MaterialSourceEntryInfo, MaterialTextureSourceInfo,
};
use crate::material_xml_info::{
    MaterialAttributeInfo, MaterialAttributeKindInfo, MaterialContentInfo, MaterialElementInfo,
    MaterialElementSyntaxInfo, MaterialParameterInfo,
};

impl MaterialLibrarySourceInfo<'_> {
    /// Owns this source description for a longer-lived package manifest.
    #[must_use]
    pub fn into_owned(self) -> MaterialLibrarySourceInfo<'static> {
        MaterialLibrarySourceInfo {
            encoding: self.encoding,
            root: self.root.map(MaterialElementInfo::into_owned),
            losses: self.losses,
            entries: self
                .entries
                .into_iter()
                .map(|entry| MaterialSourceEntryInfo {
                    ordinal: entry.ordinal,
                    element_name: owned(entry.element_name),
                    disposition: entry.disposition,
                })
                .collect(),
        }
    }
}

impl MaterialAuthoredInfo<'_> {
    /// Owns material metadata without recomputing its source or rendering interpretation.
    #[must_use]
    pub fn into_owned(self) -> MaterialAuthoredInfo<'static> {
        MaterialAuthoredInfo {
            other_attributes: self.other_attributes.into_iter().map(parameter).collect(),
            public_params: self.public_params.into_iter().map(parameter).collect(),
            layers: self
                .layers
                .into_iter()
                .map(|layer| MaterialLayerSourceInfo {
                    attributes: layer.attributes.into_iter().map(parameter).collect(),
                })
                .collect(),
            elements: self
                .elements
                .into_iter()
                .map(MaterialElementInfo::into_owned)
                .collect(),
            truncated: self.truncated.into_iter().map(owned).collect(),
            textures: self
                .textures
                .into_iter()
                .map(|slot| MaterialTextureSourceInfo {
                    map: owned(slot.map),
                    file: owned(slot.file),
                    attributes: slot.attributes.into_iter().map(parameter).collect(),
                    children: slot
                        .children
                        .into_iter()
                        .map(MaterialElementInfo::into_owned)
                        .collect(),
                })
                .collect(),
        }
    }
}

impl MaterialElementInfo<'_> {
    fn into_owned(self) -> MaterialElementInfo<'static> {
        MaterialElementInfo {
            name: owned(self.name),
            attributes: self.attributes.into_iter().map(parameter).collect(),
            children: self.children.into_iter().map(Self::into_owned).collect(),
            syntax: MaterialElementSyntaxInfo {
                ordinal: self.syntax.ordinal,
                qualified_name: owned(self.syntax.qualified_name),
                namespace: self.syntax.namespace.map(owned),
                attributes: self
                    .syntax
                    .attributes
                    .into_iter()
                    .map(|attribute| MaterialAttributeInfo {
                        name: owned(attribute.name),
                        value: owned(attribute.value),
                        lexeme: attribute.lexeme.map(owned),
                        kind: match attribute.kind {
                            MaterialAttributeKindInfo::Attribute { namespace } => {
                                MaterialAttributeKindInfo::Attribute {
                                    namespace: namespace.map(owned),
                                }
                            }
                            MaterialAttributeKindInfo::Namespace { prefix } => {
                                MaterialAttributeKindInfo::Namespace {
                                    prefix: prefix.map(owned),
                                }
                            }
                            MaterialAttributeKindInfo::Unsupported => {
                                MaterialAttributeKindInfo::Unsupported
                            }
                        },
                    })
                    .collect(),
                content: self.syntax.content.into_iter().map(content).collect(),
                binary_text: self.syntax.binary_text.map(owned),
            },
        }
    }
}

fn content(content: MaterialContentInfo<'_>) -> MaterialContentInfo<'static> {
    match content {
        MaterialContentInfo::Text { value, lexeme } => MaterialContentInfo::Text {
            value: owned(value),
            lexeme: owned(lexeme),
        },
        MaterialContentInfo::Element { ordinal } => MaterialContentInfo::Element { ordinal },
        MaterialContentInfo::Comment { value } => MaterialContentInfo::Comment {
            value: owned(value),
        },
        MaterialContentInfo::ProcessingInstruction { target, value } => {
            MaterialContentInfo::ProcessingInstruction {
                target: owned(target),
                value: value.map(owned),
            }
        }
        MaterialContentInfo::Unsupported => MaterialContentInfo::Unsupported,
    }
}

fn parameter(parameter: MaterialParameterInfo<'_>) -> MaterialParameterInfo<'static> {
    MaterialParameterInfo {
        name: owned(parameter.name),
        value: owned(parameter.value),
    }
}

fn owned(value: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(value.into_owned())
}
