// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{Element, SourceContent, SourceDeclarationKind, SourceSyntax};

use crate::MaterialError;
use crate::mtl_source::{
    AuthoredAttribute, AuthoredAttributeKind, AuthoredContent, ElementAuthored,
};

pub(crate) struct Syntax<'input> {
    name: &'input str,
    namespace: Option<&'input str>,
    attributes: Vec<Attribute<'input>>,
    content: Vec<Content<'input>>,
    binary_text: Option<&'input str>,
}

struct Attribute<'input> {
    name: &'input str,
    value: &'input str,
    lexeme: Option<&'input str>,
    kind: AttributeKind<'input>,
}

#[derive(Clone, Copy)]
enum AttributeKind<'input> {
    Attribute { namespace: Option<&'input str> },
    Namespace { prefix: Option<&'input str> },
}

enum Content<'input> {
    Text {
        value: &'input str,
        lexeme: &'input str,
    },
    Element {
        ordinal: usize,
    },
    Comment {
        value: &'input str,
    },
    ProcessingInstruction {
        target: &'input str,
        value: Option<&'input str>,
    },
}

impl Syntax<'_> {
    pub(crate) fn owned_bytes(&self) -> Option<usize> {
        let mut size = self
            .name
            .len()
            .checked_add(self.namespace.map_or(0, str::len))?;
        size = size.checked_add(self.binary_text.map_or(0, str::len))?;
        for attribute in &self.attributes {
            size = size.checked_add(size_of::<AuthoredAttribute>())?;
            size = size
                .checked_add(attribute.name.len())?
                .checked_add(attribute.value.len())?;
            size = size.checked_add(attribute.lexeme.map_or(0, str::len))?;
            let extra = match attribute.kind {
                AttributeKind::Attribute { namespace } => namespace.map_or(0, str::len),
                AttributeKind::Namespace { prefix } => prefix.map_or(0, str::len),
            };
            size = size.checked_add(extra)?;
        }
        for content in &self.content {
            size = size.checked_add(size_of::<AuthoredContent>())?;
            let extra = match content {
                Content::Text { value, lexeme } => value.len().checked_add(lexeme.len())?,
                Content::Element { .. } => 0,
                Content::Comment { value } => value.len(),
                Content::ProcessingInstruction { target, value } => {
                    target.len().checked_add(value.map_or(0, str::len))?
                }
            };
            size = size.checked_add(extra)?;
        }
        Some(size)
    }

    pub(crate) fn into_owned(self, ordinal: Option<usize>) -> ElementAuthored {
        ElementAuthored {
            ordinal,
            qualified_name: self.name.to_owned(),
            namespace: self.namespace.map(str::to_owned),
            attributes: self
                .attributes
                .into_iter()
                .map(Attribute::into_owned)
                .collect(),
            content: self.content.into_iter().map(Content::into_owned).collect(),
            binary_text: self.binary_text.map(str::to_owned),
        }
    }
}

impl Attribute<'_> {
    fn into_owned(self) -> AuthoredAttribute {
        let kind = match self.kind {
            AttributeKind::Attribute { namespace } => AuthoredAttributeKind::Attribute {
                namespace: namespace.map(str::to_owned),
            },
            AttributeKind::Namespace { prefix } => AuthoredAttributeKind::Namespace {
                prefix: prefix.map(str::to_owned),
            },
        };
        AuthoredAttribute {
            name: self.name.to_owned(),
            value: self.value.to_owned(),
            lexeme: self.lexeme.map(str::to_owned),
            kind,
        }
    }
}

impl Content<'_> {
    fn into_owned(self) -> AuthoredContent {
        match self {
            Self::Text { value, lexeme } => AuthoredContent::Text {
                value: value.to_owned(),
                lexeme: lexeme.to_owned(),
            },
            Self::Element { ordinal } => AuthoredContent::Element { ordinal },
            Self::Comment { value } => AuthoredContent::Comment {
                value: value.to_owned(),
            },
            Self::ProcessingInstruction { target, value } => {
                AuthoredContent::ProcessingInstruction {
                    target: target.to_owned(),
                    value: value.map(str::to_owned),
                }
            }
        }
    }
}

pub(crate) fn cryxml(element: Element<'_>) -> Result<Syntax<'_>, MaterialError> {
    source(&SourceSyntax::from_cryxml_element(element), None)
}

pub(crate) fn text<'document, 'input: 'document>(
    element: roxmltree::Node<'document, 'input>,
) -> Result<Syntax<'document>, MaterialError> {
    let syntax = SourceSyntax::from_text_element(element).map_err(|_| MaterialError::SourceSpan)?;
    source(&syntax, element.tag_name().namespace())
}

fn source<'document>(
    syntax: &SourceSyntax<'document>,
    namespace: Option<&'document str>,
) -> Result<Syntax<'document>, MaterialError> {
    let mut attributes = Vec::new();
    for declaration in syntax.declarations() {
        let declaration = declaration.map_err(|_| MaterialError::SourceSpan)?;
        let kind = match declaration.kind() {
            SourceDeclarationKind::Attribute { namespace } => {
                AttributeKind::Attribute { namespace }
            }
            SourceDeclarationKind::Namespace { prefix } => AttributeKind::Namespace { prefix },
            _ => return Err(MaterialError::SourceSpan),
        };
        attributes.push(Attribute {
            name: declaration.qualified_name(),
            value: declaration.value(),
            lexeme: declaration.lexeme(),
            kind,
        });
    }
    Ok(Syntax {
        name: syntax.qualified_name(),
        namespace,
        attributes,
        content: content(syntax)?,
        binary_text: syntax.binary_text().map(|text| text.value()),
    })
}

fn content<'document>(
    syntax: &SourceSyntax<'document>,
) -> Result<Vec<Content<'document>>, MaterialError> {
    let mut content = Vec::new();
    let mut ordinal = 0_usize;
    for item in syntax.content().map_err(|_| MaterialError::SourceSpan)? {
        let value = match item.map_err(|_| MaterialError::SourceSpan)? {
            SourceContent::Element(_) => {
                let item = Content::Element { ordinal };
                ordinal = ordinal.checked_add(1).ok_or(MaterialError::SourceSpan)?;
                item
            }
            SourceContent::Text(text) => Content::Text {
                value: text.value(),
                lexeme: text.lexeme().ok_or(MaterialError::SourceSpan)?,
            },
            SourceContent::Comment(text) => Content::Comment {
                value: text.value(),
            },
            SourceContent::ProcessingInstruction { target, value, .. } => {
                Content::ProcessingInstruction { target, value }
            }
            _ => return Err(MaterialError::SourceSpan),
        };
        content.push(value);
    }
    Ok(content)
}
