// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceContent, SourceContentPosition, SourceContents,
    SourceDeclaration, SourceDocument, SourceElement, SourceElementPosition, SourceSpanError,
    SourceSyntax, SourceText,
};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use crate::audio_control_fields::{AtlFamily, AtlField, AtlScalar};
use crate::audio_control_graph::Site;
use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlPayload, AtlPosition};
use crate::audio_control_reference as references;
use crate::input_budget::InputBudget;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryLookup, MediaError, MediaWriter, Status, Subresource, origin_error};

pub(crate) struct Projection<'source, 'directory> {
    pub(crate) input: &'source MediaOriginInput,
    pub(crate) budget: &'source InputBudget,
    pub(crate) directory: &'source DirectoryLookup<'directory>,
    pub(crate) entry: usize,
}

pub(crate) enum Failure {
    Span,
    Fatal(MediaError),
}

impl From<SourceSpanError> for Failure {
    fn from(_cause: SourceSpanError) -> Self {
        Self::Span
    }
}

impl From<MediaError> for Failure {
    fn from(cause: MediaError) -> Self {
        Self::Fatal(cause)
    }
}

struct Frame<'document> {
    parent: Option<u64>,
    children: SourceContents<'document>,
    sibling: u64,
    preload: Option<u64>,
}

struct ElementRoute {
    location: wire::AtlLocation,
    index: u64,
    preload: Option<u64>,
}

#[derive(Clone, Copy)]
struct Definition {
    family: Option<AtlFamily>,
    preload: Option<u64>,
}

impl Projection<'_, '_> {
    pub(crate) fn tree(
        &self,
        source: &SourceDocument<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let typed = source.root().name() == "ATLConfig";
        let mut stack = Vec::new();
        reserve(&mut stack, writer)?;
        stack.push(Frame {
            parent: None,
            children: source.content(),
            sibling: 0,
            preload: None,
        });
        let mut next = 0_u64;
        while let Some(frame) = stack.last_mut() {
            let Some(item) = frame.children.next() else {
                stack.pop();
                continue;
            };
            self.work(writer)?;
            let location = wire::AtlLocation {
                parent: frame.parent.map(DecimalU64::new),
                sibling_ordinal: DecimalU64::new(frame.sibling),
            };
            frame.sibling = advance(frame.sibling)?;
            let index = next;
            next = advance(next)?;
            match item? {
                SourceContent::Element(element) => {
                    let route = ElementRoute {
                        location,
                        index,
                        preload: frame.preload,
                    };
                    let child = self.element(element, route, typed, writer)?;
                    reserve(&mut stack, writer)?;
                    stack.push(child);
                }
                other => self.content(&other, location, index, writer)?,
            }
        }
        Ok(())
    }

    fn element<'document>(
        &self,
        element: SourceElement<'document>,
        route: ElementRoute,
        typed: bool,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Frame<'document>, Failure> {
        let ElementRoute {
            location,
            index,
            preload,
        } = route;
        let syntax = element.syntax()?;
        let position = element_position(&element.position())?;
        self.write(
            position,
            "element",
            &Subresource::new(wire::ELEMENT, vec![index]),
            &AtlPayload::Element(wire::AtlElement {
                qualified_name: syntax.qualified_name(),
                namespace: element.namespace(),
                location,
                position,
            }),
            writer,
        )?;
        let family = if typed {
            AtlFamily::from_tag(element.name())
        } else {
            None
        };
        let preload = if family == Some(AtlFamily::PreloadRequest) {
            Some(index)
        } else {
            preload
        };
        let definition = Definition { family, preload };
        self.declarations(&syntax, definition, position, index, writer)?;
        if let Some(text) = syntax.binary_text() {
            let position = content_position(text.position())?;
            self.write(
                position,
                "content",
                &Subresource::new(wire::BINARY_TEXT, vec![index]),
                &AtlPayload::BinaryText(wire::AtlBinaryText {
                    position,
                    value: text.value(),
                }),
                writer,
            )?;
        }
        Ok(Frame {
            parent: Some(index),
            children: syntax.content()?,
            sibling: 0,
            preload,
        })
    }

    fn declarations(
        &self,
        syntax: &SourceSyntax<'_>,
        definition: Definition,
        position: AtlPosition,
        index: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        if let Some(family) = definition.family {
            self.write(
                position,
                "element",
                &Subresource::new(wire::DEFINITION, vec![index]),
                &AtlPayload::Definition(wire::AtlDefinition { family }),
                writer,
            )?;
        }
        for (ordinal, declaration) in syntax.declarations().enumerate() {
            self.work(writer)?;
            self.declaration(&declaration?, definition, [index, ordinal as u64], writer)?;
        }
        Ok(())
    }

    fn declaration(
        &self,
        declaration: &SourceDeclaration<'_>,
        definition: Definition,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let position = declaration_position(declaration.position())?;
        self.write(
            position,
            "declaration",
            &Subresource::new(wire::DECLARATION, indices.to_vec()),
            &AtlPayload::Declaration(wire::AtlDeclaration {
                qualified_name: declaration.qualified_name(),
                value: declaration.value(),
                lexeme: declaration.lexeme(),
                kind: wire::AtlDeclarationKind::try_from(declaration.kind())?,
                position,
            }),
            writer,
        )?;
        if let Some(family) = definition.family
            && let Some(field) = AtlField::from_attribute(family, declaration.qualified_name())
        {
            let typed_value = field.value(family, declaration.value());
            let short_id = match &typed_value {
                AtlScalar::WwiseEvent { id, .. } => Some(*id),
                _ => None,
            };
            let site = Site {
                entry: self.entry,
                indices,
                position,
            };
            references::authored_name(
                self.directory,
                self.input,
                references::NameDeclaration {
                    family,
                    field,
                    name: declaration.value(),
                    short_id,
                    site,
                    preload: definition.preload,
                },
                writer,
            )?;
            let prepared = origins::observation(
                self.input,
                position,
                "declaration",
                SourceUseRole::Conversion,
                writer,
            )?;
            prepared.write(
                writer,
                &Subresource::new(wire::FIELD, indices.to_vec()),
                Status::Complete,
                &AtlPayload::Field(wire::AtlFieldRecord { field, typed_value }),
                &[],
            )?;
        }
        Ok(())
    }

    fn content(
        &self,
        content: &SourceContent<'_>,
        location: wire::AtlLocation,
        index: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        use wire::AtlContentValue;

        let (position, content) = match content {
            SourceContent::Text(text) => (
                content_position(text.position())?,
                AtlContentValue::Text {
                    value: text.value(),
                    lexeme: lexeme(text)?,
                },
            ),
            SourceContent::Comment(text) => (
                content_position(text.position())?,
                AtlContentValue::Comment {
                    value: text.value(),
                    lexeme: lexeme(text)?,
                },
            ),
            SourceContent::ProcessingInstruction {
                target,
                value,
                position,
                ..
            } => (
                AtlPosition::text(position),
                AtlContentValue::ProcessingInstruction {
                    target,
                    value: *value,
                },
            ),
            _ => return Err(Failure::Span),
        };
        self.write(
            position,
            "content",
            &Subresource::new(wire::CONTENT, vec![index]),
            &AtlPayload::Content(wire::AtlContent {
                location,
                position,
                content,
            }),
            writer,
        )?;
        Ok(())
    }

    fn write(
        &self,
        position: AtlPosition,
        field: &str,
        subresource: &Subresource,
        payload: &AtlPayload<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let prepared =
            origins::observation(self.input, position, field, SourceUseRole::Payload, writer)?;
        prepared.write(writer, subresource, Status::Complete, payload, &[])
    }

    pub(crate) fn document_observed(
        &self,
        size: u64,
        complete: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let evidence = origins::file(self.input, size, SourceUseRole::Selector, writer)?;
        self.directory
            .index
            .atl_controls
            .document(self.entry, complete, evidence)
    }

    fn work(&self, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        self.budget
            .metadata(0, 1, writer.limits(), writer.checkpoint)
    }
}

fn reserve<T>(stack: &mut Vec<T>, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
    stack.try_reserve(1).map_err(|cause| MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
    })
}

fn advance(value: u64) -> Result<u64, MediaError> {
    value.checked_add(1).ok_or_else(origin_error::counter)
}

fn lexeme<'document>(text: &SourceText<'document>) -> Result<&'document str, Failure> {
    text.lexeme().ok_or(Failure::Span)
}

const fn element_position(position: &SourceElementPosition) -> Result<AtlPosition, Failure> {
    Ok(match position {
        SourceElementPosition::Text(range) => AtlPosition::text(range),
        SourceElementPosition::CryXml(node_index) => AtlPosition::CryXmlNode {
            node_index: *node_index,
        },
        _ => return Err(Failure::Span),
    })
}

const fn declaration_position(position: &SourceAttributePosition) -> Result<AtlPosition, Failure> {
    Ok(match position {
        SourceAttributePosition::Text(range) => AtlPosition::text(range),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
            ..
        } => AtlPosition::CryXmlAttribute {
            node_index: *node_index,
            attribute_index: *attribute_index,
        },
        _ => return Err(Failure::Span),
    })
}

const fn content_position(position: &SourceContentPosition) -> Result<AtlPosition, Failure> {
    Ok(match position {
        SourceContentPosition::Text(range) => AtlPosition::text(range),
        SourceContentPosition::CryXml(node_index) => AtlPosition::CryXmlContent {
            node_index: *node_index,
        },
        _ => return Err(Failure::Span),
    })
}
