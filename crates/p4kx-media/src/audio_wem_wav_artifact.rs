// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_extraction::{DecimalU64, WavReport};

use super::ArtifactEvidence;
use crate::audio_payload::{WemLoop, WrittenSignal};
use crate::{Artifact, ArtifactExtension, MediaError, MediaWriter};

pub(super) struct WavArtifact {
    artifact: Artifact,
    pub(super) report: WavReport,
}

impl WavArtifact {
    pub(super) fn signal(self) -> WrittenSignal {
        WrittenSignal {
            artifact: self.artifact,
            channels: self.report.channels,
            sample_rate: self.report.sample_rate,
            sample_count: DecimalU64::new(self.report.sample_count),
            loop_points: self
                .report
                .loop_points
                .map(|points| WemLoop::from_frames(points.start, points.end)),
        }
    }
}

pub(super) fn write(
    report: WavReport,
    evidence: &ArtifactEvidence,
    writer: &mut MediaWriter<'_>,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), MediaError>,
) -> Result<WavArtifact, MediaError> {
    let manifest = evidence.manifest(writer)?;
    let artifact = writer.converted_artifact_with_origins(
        ArtifactExtension::new("wav")?,
        &manifest,
        encode,
    )?;
    if artifact.size != report.size {
        return Err(MediaError::Contract(
            "WAV report and verified artifact sizes differ".into(),
        ));
    }
    Ok(WavArtifact { artifact, report })
}
