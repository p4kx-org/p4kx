// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_cryxml::Element;

use crate::MaterialError;
use crate::mtl_source::{
    AuthoredAttribute, AuthoredAttributeKind, AuthoredContent, ElementAuthored,
};
use crate::mtl_xml_tokens;

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

pub(crate) fn cryxml(element: Element<'_>) -> Syntax<'_> {
    let attributes = element
        .attributes()
        .map(|attribute| {
            let name = attribute.name();
            let kind = if name == "xmlns" {
                AttributeKind::Namespace { prefix: None }
            } else if let Some(prefix) = name.strip_prefix("xmlns:") {
                AttributeKind::Namespace {
                    prefix: Some(prefix),
                }
            } else {
                AttributeKind::Attribute { namespace: None }
            };
            Attribute {
                name,
                value: attribute.value(),
                lexeme: None,
                kind,
            }
        })
        .collect();
    Syntax {
        name: element.name(),
        namespace: None,
        attributes,
        content: element
            .children()
            .enumerate()
            .map(|(ordinal, _)| Content::Element { ordinal })
            .collect(),
        binary_text: element.content(),
    }
}

pub(crate) fn text<'document, 'input: 'document>(
    element: roxmltree::Node<'document, 'input>,
) -> Result<Syntax<'document>, MaterialError> {
    let source = element.document().input_text();
    let tag = mtl_xml_tokens::start_tag(source, element.range())?;
    let namespaces: HashMap<Option<&str>, &str> = element
        .namespaces()
        .map(|namespace| (namespace.name(), namespace.uri()))
        .collect();
    let mut decoded = element.attributes();
    let mut attributes = Vec::new();
    for token in tag.attributes {
        let namespace_prefix = if token.name == "xmlns" {
            Some(None)
        } else {
            token.name.strip_prefix("xmlns:").map(Some)
        };
        let (value, kind) = if let Some(prefix) = namespace_prefix {
            let value = if token.value.is_empty() {
                ""
            } else {
                *namespaces.get(&prefix).ok_or(MaterialError::SourceSpan)?
            };
            (value, AttributeKind::Namespace { prefix })
        } else {
            let attribute = decoded.next().ok_or(MaterialError::SourceSpan)?;
            let local = token
                .name
                .rsplit_once(':')
                .map_or(token.name, |(_, local)| local);
            if attribute.name() != local {
                return Err(MaterialError::SourceSpan);
            }
            (
                attribute.value(),
                AttributeKind::Attribute {
                    namespace: attribute.namespace(),
                },
            )
        };
        attributes.push(Attribute {
            name: token.name,
            value,
            lexeme: Some(token.value),
            kind,
        });
    }
    if decoded.next().is_some() {
        return Err(MaterialError::SourceSpan);
    }
    Ok(Syntax {
        name: tag.name,
        namespace: element.tag_name().namespace(),
        attributes,
        content: content(element, tag.name)?,
        binary_text: None,
    })
}

fn content<'document, 'input: 'document>(
    element: roxmltree::Node<'document, 'input>,
    qualified_name: &str,
) -> Result<Vec<Content<'document>>, MaterialError> {
    let source = element.document().input_text();
    let mut content = Vec::new();
    let mut ordinal = 0;
    let range = element.range();
    let element_text = source.get(range.clone()).ok_or(MaterialError::SourceSpan)?;
    let closing = closing_tag_start(element_text, qualified_name)? + range.start;
    let mut children = element.children().peekable();
    while let Some(child) = children.next() {
        if child.is_element() {
            content.push(Content::Element { ordinal });
            ordinal = ordinal.checked_add(1).ok_or(MaterialError::SourceSpan)?;
        } else if child.is_text() {
            let value = child.text().unwrap_or_default();
            let end = children.peek().map_or(closing, |next| next.range().start);
            let lexeme = source
                .get(child.range().start..end)
                .ok_or(MaterialError::SourceSpan)?;
            content.push(Content::Text { value, lexeme });
        } else if child.is_comment() {
            content.push(Content::Comment {
                value: child.text().unwrap_or_default(),
            });
        } else if let Some(instruction) = child.pi() {
            content.push(Content::ProcessingInstruction {
                target: instruction.target,
                value: instruction.value,
            });
        }
    }
    Ok(content)
}

fn closing_tag_start(source: &str, qualified_name: &str) -> Result<usize, MaterialError> {
    if source.ends_with("/>") {
        return Ok(source.len());
    }
    let end = source.strip_suffix('>').ok_or(MaterialError::SourceSpan)?;
    let end = end.trim_end_matches(|character: char| character.is_ascii_whitespace());
    let before_name = end
        .strip_suffix(qualified_name)
        .ok_or(MaterialError::SourceSpan)?;
    let before_close = before_name
        .strip_suffix("</")
        .ok_or(MaterialError::SourceSpan)?;
    Ok(before_close.len())
}
