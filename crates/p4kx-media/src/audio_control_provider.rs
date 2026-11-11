// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadError, SourceReadOptions};
use p4kx_extraction::sound::wwise_path;

use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlDocument, AtlFailure, AtlPayload};
use crate::audio_control_projection::{Failure, Projection as AtlProjection};
use crate::audio_record_origins as records;
use crate::required_reader::RequiredReader;
use crate::{
    Capability, DirectoryContext, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter,
    Projection, Provider, Severity, SourceContext, Status, Subresource, error,
};

pub(crate) struct AudioControlProvider;

impl Provider for AudioControlProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::AudioControl,
            "p4kx.audio-control",
            "1.0.0",
            "Exact selected ATL XML and CryXml bytes, source-ordered authored syntax, typed control families, and observed declarations.",
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
        if !wwise_path::is_atl_definition(context.entry.path()) {
            return Ok(Projection::Skipped);
        }
        let input = context.origin_input(writer)?;
        let budget = &context.directory.input_state()?.budget;
        let projection = AtlProjection {
            input: &input,
            budget,
            directory: context.directory,
            entry: context.entry.index(),
        };
        let bytes = read(context, writer)?;
        let source = match SourceDocument::parse(&bytes, SourceReadOptions::default()) {
            Ok(source) => source,
            Err(cause) => {
                projection.document_observed(context.source.size, false, writer)?;
                failure(&projection, context.source.size, reason(&cause), writer)?;
                return Ok(Projection::Written);
            }
        };
        writer.checkpoint()?;
        document(&projection, &source, context.source.size, writer)?;
        match projection.tree(&source, writer) {
            Ok(()) => {
                projection.document_observed(context.source.size, !source.is_lossy(), writer)?;
            }
            Err(Failure::Span) => {
                projection.document_observed(context.source.size, false, writer)?;
                failure(
                    &projection,
                    context.source.size,
                    AtlFailure::SourceSpan,
                    writer,
                )?;
            }
            Err(Failure::Fatal(cause)) => return Err(cause),
        }
        Ok(Projection::Written)
    }
}

fn read(context: &SourceContext<'_>, writer: &MediaWriter<'_>) -> Result<Vec<u8>, MediaError> {
    let budget = &context.directory.input_state()?.budget;
    let limits = writer.limits();
    let size = context.source.size;
    if size > budget.remaining_bytes(limits) {
        return Err(MediaError::Limit {
            budget: "source_bytes",
            limit: limits.source_bytes,
        });
    }
    let native_size = usize::try_from(size)
        .map_err(|_| MediaError::Contract("ATL source exceeds native address width".into()))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(native_size).map_err(|cause| {
        error::io(
            Path::new(context.entry.path()),
            io::Error::new(io::ErrorKind::OutOfMemory, cause),
        )
    })?;
    bytes.resize(native_size, 0);
    let mut reader = RequiredReader::open(
        context.payload,
        context.entry.path(),
        size,
        budget,
        writer.checkpoint,
        limits,
    )?;
    reader
        .read_exact(&mut bytes)
        .map_err(|cause| error::io(Path::new(context.entry.path()), cause))?;
    Ok(bytes)
}

fn document(
    projection: &AtlProjection<'_, '_>,
    source: &SourceDocument<'_>,
    size: u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let repaired = source.is_lossy();
    let status = if repaired {
        Status::Partial
    } else {
        Status::Complete
    };
    let mut prepared = origins::document(projection.input, size, SourceUseRole::Payload, writer)?;
    let mut issues = Vec::new();
    if repaired {
        issues.push(MediaIssue::new(
            "audio-control.repaired-strings",
            "document",
            Severity::Warning,
            "CryXml string decoding repaired bytes in the retained source.",
        ));
        records::subtree(
            &mut prepared,
            "/issues",
            origins::file(projection.input, size, SourceUseRole::Conversion, writer)?,
            writer,
        )?;
    }
    prepared.write(
        writer,
        &Subresource::new(wire::DOCUMENT, Vec::new()),
        status,
        &AtlPayload::Document(AtlDocument {
            format: source.format().into(),
            root_is_atl_config: source.root().name() == "ATLConfig",
            repaired_string_bytes: repaired,
        }),
        &issues,
    )
}

fn failure(
    projection: &AtlProjection<'_, '_>,
    size: u64,
    reason: AtlFailure,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared =
        origins::document(projection.input, size, SourceUseRole::Conversion, writer)?;
    records::subtree(
        &mut prepared,
        "/issues",
        origins::file(projection.input, size, SourceUseRole::Conversion, writer)?,
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
            "The retained ATL XML source cannot be projected completely.",
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
