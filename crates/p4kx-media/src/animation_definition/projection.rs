// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContents, SourceDeclaration, SourceDocument, SourceElement,
    SourceSpanError, SourceSyntax,
};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::blend_reference_index::BlendIndex;
use super::blend_references;
use super::graph_field::GraphField;
use super::graph_source::GraphRole;
use super::links::{self, SubDatabaseSelector};
use super::named_clip::Filepath;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload};
#[path = "projection_branch.rs"]
mod branch;

use super::sub_database_selectors::{BranchSelector, Scopes};
use super::timeline::{TimelineIdentity, TimelineNode};
use crate::audio_control_payload::{
    AtlBinaryText, AtlContent, AtlDeclaration, AtlElement, AtlLocation, AtlPosition,
};
use crate::origin_input::MediaOriginInput;
use crate::xml_source_projection::{
    content_position, declaration_position, element_position, source_content,
};
use crate::{MediaError, MediaWriter, SourceContext, Status, Subresource, origin_error};
use branch::{AttachedSelector, BranchSite};

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
    branch: Option<BranchSite>,
    siblings: SourceContents<'document>,
    next_sibling: u64,
}

impl<'document> Frame<'document> {
    fn root(source: &'document SourceDocument<'_>) -> Self {
        Self {
            parent: None,
            parent_name: None,
            filepath: None,
            timeline: TimelineNode::Other,
            graph: None,
            branch: None,
            siblings: source.content(),
            next_sibling: 0,
        }
    }
}

struct ElementDeclarations<'element, 'document> {
    element: SourceElement<'document>,
    syntax: &'element SourceSyntax<'document>,
    index: u64,
    timeline: TimelineNode,
    graph: Option<GraphRole>,
    sub_database: Option<&'element SubDatabaseSelector>,
    branch_selector: Option<AttachedSelector<'element, 'document>>,
}

impl DefinitionProjection<'_, '_> {
    pub(super) fn tree(
        &self,
        source: &SourceDocument<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<u64, Failure> {
        let budget = &self.context.directory.input_state()?.budget;
        let mut stack = vec![Frame::root(source)];
        let mut index = 0_u64;
        let mut blend = if matches!(self.family, DefinitionFamily::Bspace) && !source.is_lossy() {
            Some(BlendIndex::default())
        } else {
            None
        };
        let scopes = Scopes::new(source.root());
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
                    let child = self.element(
                        element,
                        frame,
                        (index, location),
                        (blend.as_mut(), &scopes),
                        writer,
                    )?;
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
        if let Some(blend) = blend {
            blend_references::write(self, &blend, writer)?;
        }
        Ok(index)
    }

    fn element<'document>(
        &self,
        element: SourceElement<'document>,
        parent: &mut Frame<'document>,
        (index, location): (u64, AtlLocation),
        (mut blend, scopes): (Option<&mut BlendIndex<'document>>, &Scopes<'document>),
        writer: &mut MediaWriter<'_>,
    ) -> Result<Frame<'document>, Failure> {
        let syntax = element.syntax()?;
        let site = TimelineIdentity {
            index,
            position: element_position(&element.position())?,
        };
        let (graph, sub_database) =
            self.graph_element(element, parent, site, blend.as_deref_mut(), writer)?;
        let timeline = TimelineNode::at(parent.timeline, self.family, element, parent.parent, site);
        branch::positioned_element(
            self,
            element,
            syntax.qualified_name(),
            location,
            site,
            (timeline, graph),
            writer,
        )?;
        let branch = BranchSite::for_element(graph, index, site.position, parent.branch);
        let selector = BranchSelector::of(graph, element, self, writer)?;
        let branch_selector = branch::attach_selector(
            self,
            scopes,
            element,
            selector.as_ref(),
            branch,
            (index, site.position),
            writer,
        )?;
        self.declarations(
            &ElementDeclarations {
                element,
                syntax: &syntax,
                index,
                timeline,
                graph,
                sub_database: sub_database.as_ref(),
                branch_selector,
            },
            parent,
            blend,
            writer,
        )?;
        self.binary_text(syntax.binary_text(), index, writer)?;
        Ok(Frame {
            parent: Some(index),
            parent_name: element.namespace().is_none().then(|| element.name()),
            filepath: Filepath::for_list(self, element, parent.parent_name, writer)?,
            timeline,
            graph: graph.map(|role| (role, site.position)),
            branch,
            siblings: syntax.content()?,
            next_sibling: 0,
        })
    }

    fn graph_element(
        &self,
        element: SourceElement<'_>,
        parent: &Frame<'_>,
        TimelineIdentity { index, position }: TimelineIdentity,
        blend: Option<&mut BlendIndex<'_>>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(Option<GraphRole>, Option<SubDatabaseSelector>), Failure> {
        let parent_role = parent.graph.map(|(role, _)| role);
        let graph = GraphRole::of(self.family, parent_role, parent.parent.is_none(), element);
        if let Some(blend) = blend {
            blend.element(graph, parent_role, index, position, writer)?;
        }
        let sub_database = SubDatabaseSelector::of(
            graph,
            parent.graph,
            element,
            self,
            (index, position),
            writer,
        )?;
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
        mut blend: Option<&mut BlendIndex<'document>>,
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
            if let Some((selector, scopes, mut site)) = element.branch_selector {
                site.declaration = position;
                scopes.tokens(site, selector, &declaration, indices[1], self, writer)?;
            }
            element
                .timeline
                .write_attribute(self, &declaration, indices, position, writer)?;
            if let Some(graph) = element.graph {
                let field = GraphField::of(graph, &declaration);
                if let Some(blend) = blend.as_deref_mut() {
                    blend.declaration(&field, graph, &declaration, indices, position, writer)?;
                }
                GraphRole::write_attribute(self, field, &declaration, indices, position, writer)?;
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
