// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContents, SourceDocument, SourceElement, SourceSpanError,
};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::links;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload};
use crate::audio_control_payload::{
    AtlBinaryText, AtlContent, AtlDeclaration, AtlElement, AtlLocation, AtlPosition,
};
use crate::origin_input::MediaOriginInput;
use crate::xml_source_projection::{
    content_position, declaration_position, element_position, source_content,
};
use crate::{MediaError, MediaWriter, SourceContext, Status, Subresource, origin_error};

pub(super) struct DefinitionProjection<'a, 'source> {
    pub(super) context: &'a SourceContext<'source>,
    pub(super) input: &'a MediaOriginInput,
    pub(super) family: DefinitionFamily,
}

pub(super) enum Failure {
    Span,
    Media(MediaError),
}

impl From<SourceSpanError> for Failure {
    fn from(_: SourceSpanError) -> Self {
        Self::Span
    }
}

impl From<MediaError> for Failure {
    fn from(error: MediaError) -> Self {
        Self::Media(error)
    }
}

struct Frame<'document> {
    parent: Option<u64>,
    parent_name: Option<&'document str>,
    siblings: SourceContents<'document>,
    next_sibling: u64,
}

impl DefinitionProjection<'_, '_> {
    pub(super) fn tree(
        &self,
        source: &SourceDocument<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<u64, Failure> {
        let budget = &self.context.directory.input_state()?.budget;
        let mut stack = vec![Frame {
            parent: None,
            parent_name: None,
            siblings: source.content(),
            next_sibling: 0,
        }];
        let mut index = 0_u64;
        while let Some(frame) = stack.last_mut() {
            let Some(item) = frame.siblings.next() else {
                stack.pop();
                continue;
            };
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            let location = AtlLocation {
                parent: frame.parent.map(DecimalU64::new),
                sibling_ordinal: DecimalU64::new(frame.next_sibling),
            };
            frame.next_sibling = advance(frame.next_sibling)?;
            let parent_name = frame.parent_name;
            match item? {
                SourceContent::Element(element) => {
                    let child = self.element(element, parent_name, index, location, writer)?;
                    stack.try_reserve(1).map_err(|cause| MediaError::Io {
                        path: writer.root.clone(),
                        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
                    })?;
                    stack.push(child);
                }
                other => {
                    let (position, content) = source_content(&other)?;
                    self.write(
                        position,
                        wire::CONTENT,
                        vec![index],
                        &DefinitionPayload::Content(AtlContent {
                            location,
                            position,
                            content,
                        }),
                        writer,
                    )?;
                }
            }
            index = advance(index)?;
        }
        Ok(index)
    }

    fn element<'document>(
        &self,
        element: SourceElement<'document>,
        parent_name: Option<&str>,
        index: u64,
        location: AtlLocation,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Frame<'document>, Failure> {
        let syntax = element.syntax()?;
        let position = element_position(&element.position())?;
        self.write(
            position,
            wire::ELEMENT,
            vec![index],
            &DefinitionPayload::Element(AtlElement {
                qualified_name: syntax.qualified_name(),
                namespace: element.namespace(),
                location,
                position,
            }),
            writer,
        )?;
        let budget = &self.context.directory.input_state()?.budget;
        for (ordinal, declaration) in syntax.declarations().enumerate() {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            let declaration = declaration?;
            let position = declaration_position(declaration.position())?;
            let indices = [index, ordinal as u64];
            self.write(
                position,
                wire::DECLARATION,
                indices.to_vec(),
                &DefinitionPayload::Declaration(AtlDeclaration {
                    qualified_name: declaration.qualified_name(),
                    value: declaration.value(),
                    lexeme: declaration.lexeme(),
                    kind: declaration.kind().try_into()?,
                    position,
                }),
                writer,
            )?;
            if let Some(kind) = links::kind(self.family, element, parent_name, &declaration) {
                links::write(self, &kind, declaration.value(), position, indices, writer)?;
            }
        }
        if let Some(text) = syntax.binary_text() {
            let position = content_position(text.position())?;
            self.write(
                position,
                wire::BINARY_TEXT,
                vec![index],
                &DefinitionPayload::BinaryText(AtlBinaryText {
                    position,
                    value: text.value(),
                }),
                writer,
            )?;
        }
        Ok(Frame {
            parent: Some(index),
            parent_name: element.namespace().is_none().then(|| element.name()),
            siblings: syntax.content()?,
            next_sibling: 0,
        })
    }

    fn write(
        &self,
        position: AtlPosition,
        namespace: &str,
        indices: Vec<u64>,
        payload: &DefinitionPayload<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let proof = super::origins::position(self.input, position, SourceUseRole::Payload, writer)?;
        super::origins::record(self.input, proof, writer)?.write(
            writer,
            &Subresource::new(namespace, indices),
            Status::Complete,
            payload,
            &[],
        )
    }
}

fn advance(value: u64) -> Result<u64, MediaError> {
    value.checked_add(1).ok_or_else(origin_error::counter)
}
