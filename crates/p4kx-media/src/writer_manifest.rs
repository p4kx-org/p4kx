// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_extraction::InputScope;

use crate::MediaWriter;
use crate::artifact::BoundedLine;
use crate::origin_contract::OriginStream;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_subject::OriginSubject;
use crate::{Artifact, MediaError, Source, error, json_output, output_operation};

impl MediaWriter<'_> {
    pub(crate) fn artifact(&mut self, artifact: &Artifact) -> Result<(), MediaError> {
        self.write_artifact_manifest(artifact, None, InputScope::Payload, None)
    }

    pub(crate) fn artifact_with_input(
        &mut self,
        artifact: &Artifact,
        input: &MediaOriginInput,
        scope: InputScope,
    ) -> Result<(), MediaError> {
        self.write_artifact_manifest(artifact, Some(input), scope, None)
    }

    pub(crate) fn artifact_with_origins(
        &mut self,
        artifact: &Artifact,
        origins: &PreparedRecordOrigins,
    ) -> Result<(), MediaError> {
        self.write_artifact_manifest(artifact, None, InputScope::Payload, Some(origins))
    }

    fn write_artifact_manifest(
        &mut self,
        artifact: &Artifact,
        input: Option<&MediaOriginInput>,
        scope: InputScope,
        origins: Option<&PreparedRecordOrigins>,
    ) -> Result<(), MediaError> {
        self.checkpoint()?;
        let result = (|| {
            if self.artifacts >= self.limits.sources.saturating_add(self.limits.records) {
                return Err(MediaError::Limit {
                    budget: "artifacts",
                    limit: self.limits.sources.saturating_add(self.limits.records),
                });
            }
            let mut line = BoundedLine::new(self.limits.record_bytes);
            serde_json::to_writer(&mut line, artifact)?;
            line.write_all(b"\n")
                .map_err(|source| error::io(&self.root, source))?;
            self.reserve_output(line.bytes.len() as u64)?;
            self.emit_manifest_origin(
                OriginStream::Artifacts,
                &line.bytes,
                input,
                Some(scope),
                OriginSubject::Artifact,
                origins,
            )?;
            self.manifest
                .write_all(&line.bytes)
                .map_err(|source| error::io(&self.root, source))?;
            self.artifacts += 1;
            Ok(())
        })();
        output_operation::checked(result, self.checkpoint())
    }

    pub(crate) fn retained_with_input(
        &mut self,
        source: &Source,
        input: &MediaOriginInput,
    ) -> Result<(), MediaError> {
        self.write_source_manifest(source, Some(input))
    }

    #[cfg(test)]
    pub(crate) fn retained(&mut self, source: &Source) -> Result<(), MediaError> {
        self.write_source_manifest(source, None)
    }

    fn write_source_manifest(
        &mut self,
        source: &Source,
        input: Option<&MediaOriginInput>,
    ) -> Result<(), MediaError> {
        self.checkpoint()?;
        let mut line = BoundedLine::new(self.limits.record_bytes);
        let serialised = serde_json::to_writer(
            crate::observed_write::ObservedWrite::new(&mut line, &self.checkpoint),
            source,
        );
        json_output::finish(serialised, self.checkpoint)?;
        line.write_all(b"\n").map_err(|_| MediaError::Limit {
            budget: "record_bytes",
            limit: self.limits.record_bytes,
        })?;
        self.reserve_output(line.bytes.len() as u64)?;
        self.emit_manifest_origin(
            OriginStream::Sources,
            &line.bytes,
            input,
            None,
            OriginSubject::Source(source),
            None,
        )?;
        let result = self
            .sources
            .write_all(&line.bytes)
            .map_err(|cause| error::io(&self.root, cause));
        output_operation::checked(result, self.checkpoint())
    }
}
