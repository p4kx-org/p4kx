// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};

use super::graph_source::GraphRole;
use super::origins;
use super::payload::{
    self as wire, DefinitionDocument, DefinitionFamily, DefinitionPayload, DocumentProjection,
};
use super::projection::{DefinitionProjection, Failure};
use super::sub_database_cycles::DocumentState;
use super::sub_database_frontier::FrontierReason;
use crate::{
    Capability, DirectoryContext, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter,
    Projection, Provider, Severity, SourceContext, Status, Subresource, required_reader,
};

pub(crate) struct AnimationDefinitionProvider;

impl Provider for AnimationDefinitionProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::AnimationDefinition,
            "p4kx.animation-definition",
            "1.4.0",
            "Original character parameters, animation event definitions and Mannequin XML source.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "source-syntax",
                "Ordered source elements, declarations and content.",
            ),
            FieldGroup::new(
                "file-links",
                "Observed animation database and graph file references.",
            ),
            FieldGroup::new(
                "named-dba-bindings",
                "Physical DBA metadata candidates for exact named CAF paths.",
            ),
            FieldGroup::new(
                "event-timelines",
                "Ordered authored timelines, event attributes and exact CAF references.",
            ),
            FieldGroup::new(
                "animation-graphs",
                "Positioned Mannequin and blend-space node roles and original attributes.",
            ),
            FieldGroup::new(
                "sub-database-cycles",
                "Positive cycles in the observed, directory-local Mannequin source graph.",
            ),
            FieldGroup::new(
                "blend-space-local-references",
                "Document-local face and pseudo example candidates from separate ordinal and alias evidence.",
            ),
        ];
        capability
    }

    fn directory_complete(
        &self,
        context: &mut DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        context
            .lookup
            .index
            .animation_sub_databases
            .write(context, writer)
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let Some(family) = DefinitionFamily::of(context.entry.path()) else {
            return Ok(Projection::Skipped);
        };
        let input = context.origin_input(writer)?;
        let projection = DefinitionProjection {
            context,
            input: &input,
            family,
        };
        let bytes = required_reader::read_all(context, writer)?;
        let Ok(document) = SourceDocument::parse(&bytes, SourceReadOptions::default()) else {
            write_document(&projection, None, DocumentProjection::InvalidSource, writer)?;
            return Ok(Projection::Written);
        };
        writer.checkpoint()?;
        match projection.tree(&document, writer) {
            Ok(nodes) => {
                let state = if document.is_lossy() {
                    DocumentProjection::RepairedStrings
                } else {
                    DocumentProjection::Readable
                };
                write_document(&projection, Some((&document, nodes)), state, writer)?;
            }
            Err(Failure::Span) => {
                write_document(
                    &projection,
                    Some((&document, 0)),
                    DocumentProjection::InvalidSourceSpan,
                    writer,
                )?;
            }
            Err(Failure::Media(error)) => return Err(error),
        }
        Ok(Projection::Written)
    }
}

fn observe_sub_database(
    projection: &DefinitionProjection<'_, '_>,
    document: Option<&SourceDocument<'_>>,
    state: &DocumentProjection,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    if !matches!(projection.family, DefinitionFamily::Adb) {
        return Ok(());
    }
    let state = match state {
        DocumentProjection::Readable => {
            let root = document
                .ok_or_else(|| {
                    MediaError::Contract("readable animation source has no document".into())
                })?
                .root();
            if matches!(
                GraphRole::of(projection.family, None, true, root),
                Some(GraphRole::Database)
            ) {
                let position = crate::xml_source_projection::element_position(&root.position())
                    .map_err(|_| {
                        MediaError::Contract("invalid animation database root position".into())
                    })?;
                DocumentState::Readable(position)
            } else {
                DocumentState::Unavailable(FrontierReason::UnsupportedTargetRoot)
            }
        }
        DocumentProjection::RepairedStrings => {
            DocumentState::Unavailable(FrontierReason::RepairedTarget)
        }
        DocumentProjection::InvalidSource | DocumentProjection::InvalidSourceSpan => {
            DocumentState::Unavailable(FrontierReason::InvalidTarget)
        }
    };
    projection
        .context
        .directory
        .index
        .animation_sub_databases
        .document(
            projection.context.entry.index(),
            projection.input,
            state,
            projection.context.source.size,
            writer,
        )
}

fn write_document(
    projection: &DefinitionProjection<'_, '_>,
    source: Option<(&SourceDocument<'_>, u64)>,
    state: DocumentProjection,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    observe_sub_database(
        projection,
        source.map(|(document, _)| document),
        &state,
        writer,
    )?;
    let (status, issue) = match state {
        DocumentProjection::Readable => (Status::Complete, None),
        DocumentProjection::RepairedStrings => (
            Status::Partial,
            Some(MediaIssue::new(
                "animation-definition.repaired-strings",
                "document",
                Severity::Warning,
                "CryXml string decoding repaired bytes in the retained source.",
            )),
        ),
        DocumentProjection::InvalidSource | DocumentProjection::InvalidSourceSpan => (
            Status::Failed,
            Some(MediaIssue::new(
                "animation-definition.invalid-source",
                "document",
                Severity::Error,
                "The retained XML source cannot be projected completely.",
            )),
        ),
    };
    origins::document(
        projection.input,
        projection.context.entry.index(),
        projection.context.source.size,
        writer,
    )?
    .write(
        writer,
        &Subresource::new(wire::DOCUMENT, Vec::new()),
        status,
        &DefinitionPayload::Document(DefinitionDocument {
            family: projection.family,
            format: source.map(|(document, _)| document.format().into()),
            nodes: source.and_then(|(_, count)| {
                (status != Status::Failed).then_some(DecimalU64::new(count))
            }),
            projection: state,
        }),
        issue.as_slice(),
    )
}
