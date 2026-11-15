// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceDocument, SourceSpanError};

use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlPayload, AtlPosition};
use crate::audio_control_reference::{self as references, NameDeclaration};
use crate::audio_record_origins as records;
use crate::input_budget::InputBudget;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryLookup, MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[cfg(test)]
#[path = "audio_control_projection_census.rs"]
mod census;
#[path = "audio_control_declaration.rs"]
pub(crate) mod declaration;
#[path = "audio_control_projection_walk.rs"]
mod walk;

#[cfg(test)]
pub(crate) use self::census::census;
use self::walk::Sink;

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

impl Projection<'_, '_> {
    pub(crate) fn project(
        &self,
        source: &SourceDocument<'_>,
        size: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let mut sink = WriterSink {
            input: self.input,
            budget: self.budget,
            directory: self.directory,
            writer,
            size,
        };
        walk::project(source, self.entry, &mut sink)
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
}

struct WriterSink<'source, 'directory, 'run> {
    input: &'source MediaOriginInput,
    budget: &'source InputBudget,
    directory: &'source DirectoryLookup<'directory>,
    writer: &'source mut MediaWriter<'run>,
    size: u64,
}

impl Sink for WriterSink<'_, '_, '_> {
    fn work(&mut self) -> Result<(), MediaError> {
        self.budget
            .metadata(0, 1, self.writer.limits(), self.writer.checkpoint)
    }

    fn path(&self) -> &Path {
        &self.writer.root
    }

    fn document(&mut self, payload: wire::AtlDocument) -> Result<(), MediaError> {
        let writer = &mut *self.writer;
        let status = if payload.repaired_string_bytes {
            Status::Partial
        } else {
            Status::Complete
        };
        let mut prepared =
            origins::document(self.input, self.size, SourceUseRole::Payload, writer)?;
        let mut issues = Vec::new();
        if payload.repaired_string_bytes {
            issues.push(MediaIssue::new(
                "audio-control.repaired-strings",
                "document",
                Severity::Warning,
                "CryXml string decoding repaired bytes in the retained source.",
            ));
            records::subtree(
                &mut prepared,
                "/issues",
                origins::file(self.input, self.size, SourceUseRole::Conversion, writer)?,
                writer,
            )?;
        }
        prepared.write(
            writer,
            &Subresource::new(wire::DOCUMENT, Vec::new()),
            status,
            &AtlPayload::Document(payload),
            &issues,
        )
    }

    fn record(
        &mut self,
        position: AtlPosition,
        field: &'static str,
        role: SourceUseRole,
        subresource: &Subresource,
        payload: &AtlPayload<'_>,
    ) -> Result<(), MediaError> {
        let prepared = origins::observation(self.input, position, field, role, self.writer)?;
        prepared.write(self.writer, subresource, Status::Complete, payload, &[])
    }

    fn name(&mut self, declaration: NameDeclaration<'_>) -> Result<(), MediaError> {
        references::authored_name(self.directory, self.input, declaration, self.writer)
    }
}
