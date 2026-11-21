// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use roxmltree::{Attributes as TextAttributes, Node as TextNode};

use crate::source_attribute::SourceAttributes;
use crate::source_position::SourceAttributePosition;
use crate::source_start_tag::{AttributeToken, AttributeTokens};
use crate::source_syntax::SourceSpanError;

/// An ordinary attribute or a declaration of an XML namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceDeclarationKind<'document> {
    /// A parsed attribute and its expanded namespace when available.
    Attribute {
        /// Expanded namespace URI.
        namespace: Option<&'document str>,
    },
    /// A declaration that assigns a namespace URI to a prefix.
    Namespace {
        /// Declared prefix, absent for the default namespace.
        prefix: Option<&'document str>,
    },
}

/// One authored XML declaration in its original order.
#[derive(Debug, Clone)]
pub struct SourceDeclaration<'document> {
    name: &'document str,
    value: &'document str,
    lexeme: Option<&'document str>,
    position: SourceAttributePosition,
    kind: SourceDeclarationKind<'document>,
}

impl<'document> SourceDeclaration<'document> {
    /// Returns the authored `QName` or stored CryXml name.
    #[must_use]
    pub const fn qualified_name(&self) -> &'document str {
        self.name
    }

    /// Returns the decoded attribute value or namespace URI.
    #[must_use]
    pub const fn value(&self) -> &'document str {
        self.value
    }

    /// Returns the exact text XML value spelling between quotes.
    #[must_use]
    pub const fn lexeme(&self) -> Option<&'document str> {
        self.lexeme
    }

    /// Returns the declaration coordinate in the original input.
    #[must_use]
    pub const fn position(&self) -> &SourceAttributePosition {
        &self.position
    }

    /// Returns the category and expanded namespace or declared prefix.
    #[must_use]
    pub const fn kind(&self) -> SourceDeclarationKind<'document> {
        self.kind
    }
}

pub(crate) enum SourceDeclarations<'document> {
    Text {
        node: TextNode<'document, 'document>,
        tokens: AttributeTokens<'document>,
        attributes: TextAttributes<'document, 'document>,
    },
    CryXml(SourceAttributes<'document>),
}

impl<'document> Iterator for SourceDeclarations<'document> {
    type Item = Result<SourceDeclaration<'document>, SourceSpanError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Text {
                node,
                tokens,
                attributes,
            } => match tokens.next() {
                Some(token) => {
                    Some(token.and_then(|token| text_declaration(*node, attributes, token)))
                }
                None => attributes.next().map(|_| Err(SourceSpanError)),
            },
            Self::CryXml(attributes) => attributes.next().map(|attribute| {
                let name = attribute.name();
                Ok(SourceDeclaration {
                    name,
                    value: attribute.value(),
                    lexeme: None,
                    kind: namespace_declaration(name)
                        .unwrap_or(SourceDeclarationKind::Attribute { namespace: None }),
                    position: attribute.into_position(),
                })
            }),
        }
    }
}

fn text_declaration<'document>(
    node: TextNode<'document, 'document>,
    attributes: &mut TextAttributes<'document, 'document>,
    token: AttributeToken<'document>,
) -> Result<SourceDeclaration<'document>, SourceSpanError> {
    let (value, kind) = if let Some(kind @ SourceDeclarationKind::Namespace { prefix }) =
        namespace_declaration(token.name)
    {
        let uri = if token.value.is_empty() {
            ""
        } else {
            node.lookup_namespace_uri(prefix).ok_or(SourceSpanError)?
        };
        (uri, kind)
    } else {
        let attribute = attributes.next().ok_or(SourceSpanError)?;
        let local = token
            .name
            .rsplit_once(':')
            .map_or(token.name, |(_, local)| local);
        if local != attribute.name() {
            return Err(SourceSpanError);
        }
        (
            attribute.value(),
            SourceDeclarationKind::Attribute {
                namespace: attribute.namespace(),
            },
        )
    };
    Ok(SourceDeclaration {
        name: token.name,
        value,
        lexeme: Some(token.value),
        position: SourceAttributePosition::Text(token.position),
        kind,
    })
}

fn namespace_declaration(name: &str) -> Option<SourceDeclarationKind<'_>> {
    let prefix = if name == "xmlns" {
        None
    } else {
        Some(name.strip_prefix("xmlns:")?)
    };
    Some(SourceDeclarationKind::Namespace { prefix })
}
