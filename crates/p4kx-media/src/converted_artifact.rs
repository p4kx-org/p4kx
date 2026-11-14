// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use crate::artifact_placement;
use crate::converted_sink::{ConvertedSink, Encoded};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::output_operation;
use crate::temporary_file::TemporaryFile;
use crate::{Artifact, ArtifactExtension, MediaError, MediaWriter};

pub(crate) struct ConvertedOccurrence {
    pub(crate) artifact: Artifact,
    pub(crate) created: bool,
}

impl MediaWriter<'_> {
    /// Writes one unknown-length encoding through a bounded protected sink.
    /// The callback receives no path or file handle; returned evidence describes placed bytes.
    ///
    /// # Errors
    /// Returns original callback, sticky output, budget, cancellation, collision or cleanup failures.
    pub fn converted_artifact(
        &mut self,
        extension: ArtifactExtension<'_>,
        encode: impl FnOnce(&mut dyn Write) -> Result<(), MediaError>,
    ) -> Result<Artifact, MediaError> {
        self.encode_conversion(extension, None, encode)
            .map(|result| result.artifact)
    }

    pub(crate) fn converted_artifact_with_origins(
        &mut self,
        extension: ArtifactExtension<'_>,
        origins: &PreparedRecordOrigins,
        encode: impl FnOnce(&mut dyn Write) -> Result<(), MediaError>,
    ) -> Result<Artifact, MediaError> {
        self.converted_occurrence_with_origins(extension, origins, encode)
            .map(|result| result.artifact)
    }

    pub(crate) fn converted_occurrence_with_origins(
        &mut self,
        extension: ArtifactExtension<'_>,
        origins: &PreparedRecordOrigins,
        encode: impl FnOnce(&mut dyn Write) -> Result<(), MediaError>,
    ) -> Result<ConvertedOccurrence, MediaError> {
        origins.require_writer(self)?;
        self.encode_conversion(extension, Some(origins), encode)
    }

    fn encode_conversion(
        &mut self,
        extension: ArtifactExtension<'_>,
        origins: Option<&PreparedRecordOrigins>,
        encode: impl FnOnce(&mut dyn Write) -> Result<(), MediaError>,
    ) -> Result<ConvertedOccurrence, MediaError> {
        self.require_output_callback()?;
        let directory = self.root.join("chunks");
        let mut temporary = artifact_placement::temporary(
            self,
            &directory,
            "converted artifact staging directory is missing",
        )?;
        let encoded = {
            let mut sink = ConvertedSink::new(self, &mut temporary);
            let callback = encode(&mut sink);
            sink.finish(callback)
        };
        let result = encoded.and_then(|encoded| {
            self.checkpoint()?;
            output_operation::checked(temporary.sync_all(), self.checkpoint())?;
            self.place_conversion(extension, encoded, &mut temporary, origins)
        });
        temporary.finish(result)
    }

    fn place_conversion(
        &mut self,
        extension: ArtifactExtension<'_>,
        encoded: Encoded,
        temporary: &mut TemporaryFile<'_>,
        origins: Option<&PreparedRecordOrigins>,
    ) -> Result<ConvertedOccurrence, MediaError> {
        let artifact = Artifact {
            path: format!("chunks/{}.{}", encoded.digest.hex, extension.as_str()),
            size: encoded.size,
            digest: encoded.digest,
        };
        let created = artifact_placement::place(
            self,
            &artifact,
            temporary,
            "converted artifact content-address collision",
        )?;
        if let Some(origins) = origins {
            self.artifact_with_origins(&artifact, origins)?;
        } else {
            self.artifact(&artifact)?;
        }
        self.converted_output(created)?;
        self.checkpoint()?;
        Ok(ConvertedOccurrence { artifact, created })
    }
}

#[cfg(test)]
#[path = "converted_artifact_fault_tests.rs"]
mod fault_tests;
