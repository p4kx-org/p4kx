// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::convert::Infallible;

use p4kx_model::{
    Attributes, AuthoredAttribute, AuthoredAttributeKind, AuthoredContent, Material,
    MaterialEncoding, MaterialEntryDisposition, MaterialLibrary, MaterialSource,
    MaterialSourceLossKind, RawElement,
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
        infallible(Self::of_source_with(source, &mut || Ok(())))
    }

    /// Projects retained material values, checking before each source element and list item.
    ///
    /// # Errors
    /// Returns the caller's checkpoint error.
    pub fn of_source_with<E>(
        source: &'source MaterialSource,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        check()?;
        Ok(Self {
            encoding: encoding(source.encoding),
            root: source
                .root
                .as_ref()
                .map(|element| MaterialElementInfo::of_with(element, check))
                .transpose()?,
            losses: map_with(&source.losses, check, |loss, _| {
                Ok(MaterialSourceLossInfo {
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
            })?,
            entries: map_with(&source.entries, check, |entry, _| {
                Ok(MaterialSourceEntryInfo {
                    ordinal: entry.ordinal,
                    element_name: Cow::Borrowed(&entry.element_name),
                    disposition: disposition(entry.disposition),
                })
            })?,
        })
    }

    /// Checks projection coverage without allocating a second copy of the wire structure.
    ///
    /// # Errors
    /// Returns the caller's checkpoint error.
    pub fn source_is_complete_with<E>(
        source: &MaterialSource,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<bool, E> {
        check()?;
        if !source.losses.is_empty()
            || encoding(source.encoding) == MaterialEncodingInfo::Unsupported
        {
            return Ok(false);
        }
        let Some(root) = &source.root else {
            return Ok(false);
        };
        if !element_is_complete(root, check)? {
            return Ok(false);
        }
        for entry in &source.entries {
            check()?;
            if disposition(entry.disposition) == MaterialEntryDispositionInfo::Unsupported {
                return Ok(false);
            }
        }
        Ok(true)
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
        infallible(Self::of_with(element, &mut || Ok(())))
    }

    fn of_with<E>(
        element: &'source RawElement,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        check()?;
        let syntax = &element.authored;
        Ok(Self {
            name: Cow::Borrowed(&element.name),
            attributes: map_with(&element.attributes, check, |pair, _| Ok(parameter(pair)))?,
            children: map_with(&element.children, check, Self::of_with)?,
            syntax: MaterialElementSyntaxInfo {
                ordinal: syntax.ordinal,
                qualified_name: Cow::Borrowed(&syntax.qualified_name),
                namespace: syntax.namespace.as_deref().map(Cow::Borrowed),
                attributes: map_with(&syntax.attributes, check, |attribute, _| {
                    Ok(attribute_info(attribute))
                })?,
                content: map_with(&syntax.content, check, |content, _| {
                    Ok(content_info(content))
                })?,
                binary_text: syntax.binary_text.as_deref().map(Cow::Borrowed),
            },
        })
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

fn element_is_complete<E>(
    element: &RawElement,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    check()?;
    for child in &element.children {
        if !element_is_complete(child, check)? {
            return Ok(false);
        }
    }
    for attribute in &element.authored.attributes {
        check()?;
        if matches!(
            attribute_info(attribute).kind,
            MaterialAttributeKindInfo::Unsupported
        ) {
            return Ok(false);
        }
    }
    for content in &element.authored.content {
        check()?;
        if matches!(content_info(content), MaterialContentInfo::Unsupported) {
            return Ok(false);
        }
    }
    Ok(true)
}

const fn encoding(value: MaterialEncoding) -> MaterialEncodingInfo {
    match value {
        MaterialEncoding::Text => MaterialEncodingInfo::Text,
        MaterialEncoding::CryXml => MaterialEncodingInfo::CryXml,
        _ => MaterialEncodingInfo::Unsupported,
    }
}

const fn disposition(value: MaterialEntryDisposition) -> MaterialEntryDispositionInfo {
    match value {
        MaterialEntryDisposition::Material => MaterialEntryDispositionInfo::Material,
        MaterialEntryDisposition::EngineDefault => MaterialEntryDispositionInfo::EngineDefault,
        _ => MaterialEntryDispositionInfo::Unsupported,
    }
}

fn attribute_info(attribute: &AuthoredAttribute) -> MaterialAttributeInfo<'_> {
    MaterialAttributeInfo {
        name: Cow::Borrowed(&attribute.name),
        value: Cow::Borrowed(&attribute.value),
        lexeme: attribute.lexeme.as_deref().map(Cow::Borrowed),
        kind: match &attribute.kind {
            AuthoredAttributeKind::Attribute { namespace } => {
                MaterialAttributeKindInfo::Attribute {
                    namespace: namespace.as_deref().map(Cow::Borrowed),
                }
            }
            AuthoredAttributeKind::Namespace { prefix } => MaterialAttributeKindInfo::Namespace {
                prefix: prefix.as_deref().map(Cow::Borrowed),
            },
            _ => MaterialAttributeKindInfo::Unsupported,
        },
    }
}

fn content_info(content: &AuthoredContent) -> MaterialContentInfo<'_> {
    match content {
        AuthoredContent::Text { value, lexeme } => MaterialContentInfo::Text {
            value: Cow::Borrowed(value),
            lexeme: Cow::Borrowed(lexeme),
        },
        AuthoredContent::Element { ordinal } => MaterialContentInfo::Element { ordinal: *ordinal },
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
    }
}

fn parameters(attributes: &Attributes) -> Vec<MaterialParameterInfo<'_>> {
    attributes.iter().map(parameter).collect()
}

fn parameter((name, value): &(String, String)) -> MaterialParameterInfo<'_> {
    MaterialParameterInfo {
        name: Cow::Borrowed(name),
        value: Cow::Borrowed(value),
    }
}

fn map_with<'a, Input, Output, E, Checkpoint>(
    input: &'a [Input],
    check: &mut Checkpoint,
    mut project: impl FnMut(&'a Input, &mut Checkpoint) -> Result<Output, E>,
) -> Result<Vec<Output>, E>
where
    Checkpoint: FnMut() -> Result<(), E>,
{
    input
        .iter()
        .map(|value| {
            check()?;
            project(value, check)
        })
        .collect()
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}
