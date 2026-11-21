// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadError, SourceReadOptions};

use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlFailure, AtlPayload};
use crate::audio_control_projection::{Failure, Projection as AtlProjection};
use crate::audio_control_source::AudioControlSource;
use crate::audio_control_source::cig_audio_role::CigAudioRole;
use crate::audio_record_origins as records;
use crate::required_reader;
use crate::{
    Capability, DirectoryContext, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter,
    Projection, Provider, Severity, SourceContext, Status, Subresource,
};

#[cfg(test)]
#[path = "audio_control_witness.rs"]
mod witness;

#[cfg(test)]
#[path = "audio_control_accounting.rs"]
mod audio_control_accounting;

pub(crate) struct AudioControlProvider;

impl Provider for AudioControlProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::AudioControl,
            "p4kx.audio-control",
            "1.1.0",
            "Exact selected Audio XML and CryXml bytes, source-ordered syntax, typed control families, and authored CIG audio graphs.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "atl-document",
                "Original document type and complete typed source coordinates for XML and CryXml syntax.",
            ),
            FieldGroup::new(
                "atl-controls",
                "Authored ATL control families and established fields with original declarations.",
            ),
            FieldGroup::new(
                "radar-audio",
                "Authored radar zoom, categories and sound-event declarations.",
            ),
            FieldGroup::new(
                "cig-audio-graph",
                "Authored graph objects, logic nodes, properties, and links with exact source coordinates.",
            ),
        ];
        capability
    }

    fn directory_complete(
        &self,
        context: &mut DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        crate::audio_control_graph::rows::project(context, writer)
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let Some(kind) = AudioControlSource::from_path(context.entry.path()) else {
            return Ok(Projection::Skipped);
        };
        let input = context.origin_input(writer)?;
        let budget = &context.directory.input_state()?.budget;
        let projection = AtlProjection {
            kind,
            input: &input,
            budget,
            directory: context.directory,
            entry: context.entry.index(),
        };
        let bytes = required_reader::read_all(context, writer)?;
        project_bytes(&projection, &bytes, writer)
    }
}

fn project_bytes(
    projection: &AtlProjection<'_, '_>,
    bytes: &[u8],
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let size = bytes.len() as u64;
    let source = match SourceDocument::parse(bytes, SourceReadOptions::default()) {
        Ok(source) => source,
        Err(cause) => {
            projection.document_observed(size, false, writer)?;
            failure(projection, size, reason(&cause), writer)?;
            return Ok(Projection::Written);
        }
    };
    writer.checkpoint()?;
    if projection.kind == AudioControlSource::CigAudio
        && CigAudioRole::root(source.root()).is_none()
    {
        failure(projection, size, AtlFailure::UnexpectedCigAudioRoot, writer)?;
        return Ok(Projection::Written);
    }
    match projection.project(&source, size, writer) {
        Ok(()) => {
            projection.document_observed(size, !source.is_lossy(), writer)?;
        }
        Err(Failure::Span) => {
            projection.document_observed(size, false, writer)?;
            failure(projection, size, AtlFailure::SourceSpan, writer)?;
        }
        Err(Failure::Fatal(cause)) => return Err(cause),
    }
    Ok(Projection::Written)
}

fn failure(
    projection: &AtlProjection<'_, '_>,
    size: u64,
    reason: AtlFailure,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let kind = projection.kind;
    let mut prepared = origins::document(
        projection.input,
        kind,
        size,
        SourceUseRole::Conversion,
        writer,
    )?;
    records::subtree(
        &mut prepared,
        "/issues",
        origins::file(
            projection.input,
            kind,
            size,
            SourceUseRole::Conversion,
            writer,
        )?,
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(wire::FAILURE, Vec::new()),
        Status::Failed,
        &AtlPayload::Failure(reason),
        &[MediaIssue::new(
            "audio-control.invalid-source",
            "document",
            Severity::Error,
            match kind {
                AudioControlSource::Atl => {
                    "The retained ATL XML source cannot be projected completely."
                }
                AudioControlSource::CigAudio => {
                    "The retained CIG audio graph cannot be projected completely."
                }
            },
        )],
    )
}

const fn reason(cause: &SourceReadError) -> AtlFailure {
    match cause {
        SourceReadError::CryXml(_) => AtlFailure::BinaryOrUnknownFormat,
        SourceReadError::Utf8(_) => AtlFailure::TextEncoding,
        SourceReadError::Text(_) | SourceReadError::TextDepth(_) => AtlFailure::TextXml,
        _ => AtlFailure::UnsupportedReader,
    }
}
