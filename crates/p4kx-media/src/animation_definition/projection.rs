// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContents, SourceDeclaration, SourceDocument, SourceElement,
    SourceSpanError, SourceSyntax,
};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::graph_source::GraphRole;
use super::links::{self, SubDatabaseSelector};
use super::named_clip::Filepath;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload};
use super::timeline::{TimelineIdentity, TimelineNode};
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
    filepath: Option<Filepath<'document>>,
    timeline: TimelineNode,
    graph: Option<(GraphRole, AtlPosition)>,
    siblings: SourceContents<'document>,
    next_sibling: u64,
}

struct ElementDeclarations<'element, 'document> {
    element: SourceElement<'document>,
    syntax: &'element SourceSyntax<'document>,
    index: u64,
    timeline: TimelineNode,
    graph: Option<GraphRole>,
    sub_database: Option<&'element SubDatabaseSelector>,
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
            filepath: None,
            timeline: TimelineNode::Other,
            graph: None,
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
            match item? {
                SourceContent::Element(element) => {
                    let child = self.element(element, frame, index, location, writer)?;
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
        parent: &mut Frame<'document>,
        index: u64,
        location: AtlLocation,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Frame<'document>, Failure> {
        let syntax = element.syntax()?;
        let position = element_position(&element.position())?;
        let (graph, sub_database) =
            self.graph_element(element, parent, (index, position), writer)?;
        let timeline = TimelineNode::at(
            parent.timeline,
            self.family,
            element,
            parent.parent,
            TimelineIdentity { index, position },
        );
        self.write_element(
            AtlElement {
                qualified_name: syntax.qualified_name(),
                namespace: element.namespace(),
                location: location.clone(),
                position,
            },
            index,
            writer,
        )?;
        timeline.write(self, element, index, position, writer)?;
        if let Some(graph) = graph {
            graph.write(
                self,
                syntax.qualified_name(),
                index,
                location,
                position,
                writer,
            )?;
        }
        self.declarations(
            &ElementDeclarations {
                element,
                syntax: &syntax,
                index,
                timeline,
                graph,
                sub_database: sub_database.as_ref(),
            },
            parent,
            writer,
        )?;
        self.binary_text(syntax.binary_text(), index, writer)?;
        Ok(Frame {
            parent: Some(index),
            parent_name: element.namespace().is_none().then(|| element.name()),
            filepath: Filepath::for_list(self, element, parent.parent_name, writer)?,
            timeline,
            graph: graph.map(|role| (role, position)),
            siblings: syntax.content()?,
            next_sibling: 0,
        })
    }

    fn graph_element(
        &self,
        element: SourceElement<'_>,
        parent: &Frame<'_>,
        site: (u64, AtlPosition),
        writer: &mut MediaWriter<'_>,
    ) -> Result<(Option<GraphRole>, Option<SubDatabaseSelector>), Failure> {
        let graph = GraphRole::of(
            self.family,
            parent.graph.map(|(role, _)| role),
            parent.parent.is_none(),
            element,
        );
        let sub_database =
            SubDatabaseSelector::of(graph, parent.graph, element, self, site, writer)?;
        Ok((graph, sub_database))
    }

    fn write_element(
        &self,
        element: AtlElement<'_>,
        index: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.write(
            element.position,
            wire::ELEMENT,
            vec![index],
            &DefinitionPayload::Element(element),
            writer,
        )
    }

    fn declarations<'document>(
        &self,
        element: &ElementDeclarations<'_, 'document>,
        parent: &mut Frame<'document>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let budget = self.context.directory.input_state()?.budget;
        for (ordinal, declaration) in element.syntax.declarations().enumerate() {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            let declaration = declaration?;
            let indices = [element.index, ordinal as u64];
            let position = self.declaration(
                element.element,
                parent,
                &declaration,
                indices,
                element.sub_database,
                writer,
            )?;
            element
                .timeline
                .write_attribute(self, &declaration, indices, position, writer)?;
            if let Some(graph) = element.graph {
                graph.write_attribute(self, &declaration, indices, position, writer)?;
            }
        }
        Ok(())
    }

    fn binary_text(
        &self,
        text: Option<p4kx_extraction::cryxml::SourceText<'_>>,
        index: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        if let Some(text) = text {
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
        Ok(())
    }

    fn declaration<'document>(
        &self,
        element: SourceElement<'document>,
        parent: &mut Frame<'document>,
        declaration: &SourceDeclaration<'document>,
        indices: [u64; 2],
        sub_database: Option<&SubDatabaseSelector>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<AtlPosition, Failure> {
        let position = declaration_position(declaration.position())?;
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
        if let Some(kind) = links::kind(self.family, element, parent.parent_name, declaration) {
            let sub_database = sub_database.filter(|selector| selector.matches(declaration));
            links::write(
                self,
                &kind,
                declaration.value(),
                position,
                indices,
                sub_database,
                writer,
            )?;
        }
        if let Some(filepath) = &mut parent.filepath {
            filepath.observe(self, element, declaration, position, indices, writer)?;
        }
        Ok(position)
    }

    pub(super) fn write(
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
