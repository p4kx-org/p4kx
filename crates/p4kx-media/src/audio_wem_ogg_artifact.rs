// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_extraction::sound::WemOriginField;
use p4kx_extraction::{Codec, DecimalU64, OggReport};

use super::{ArtifactEvidence, ConversionOrigins, LOOP_FIELDS, LOOP_POINTS, OUTCOME};
use crate::audio_payload::{WemLoop, WrittenSignal};
use crate::origin_contributions::OriginContributions;
use crate::{Artifact, ArtifactExtension, MediaError, MediaWriter};

pub(super) struct OggArtifact {
    artifact: Artifact,
    pub(super) report: OggReport,
}

impl OggArtifact {
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
    codec: Codec,
    frames: u64,
    evidence: &ArtifactEvidence,
    writer: &mut MediaWriter<'_>,
    encode: impl FnOnce(&mut dyn Write) -> Result<OggReport, MediaError>,
) -> Result<OggArtifact, MediaError> {
    let manifest = evidence.manifest(writer)?;
    let mut report = None;
    let artifact = writer.converted_artifact_with_origins(
        ArtifactExtension::new("ogg")?,
        &manifest,
        |sink| {
            let written = encode(sink)?;
            if written.sample_count != frames || written.codec != codec {
                return Err(MediaError::Contract(
                    "Ogg replay differs from the checked sample count or codec".into(),
                ));
            }
            report = Some(written);
            Ok(())
        },
    )?;
    let report = report.ok_or_else(|| MediaError::Contract("missing Ogg write report".into()))?;
    if artifact.size != report.size {
        return Err(MediaError::Contract(
            "Ogg report and verified artifact sizes differ".into(),
        ));
    }
    Ok(OggArtifact { artifact, report })
}

pub(super) fn prove(
    proof: &mut ConversionOrigins<'_, '_, '_, '_>,
    channels: OriginContributions,
    frames: &OriginContributions,
    artifact: &ArtifactEvidence,
    has_loop: bool,
) -> Result<(), MediaError> {
    proof.common_success(channels)?;
    proof.field(&format!("{OUTCOME}/payload/sample_count"), frames.clone())?;
    let loop_points = join(
        proof.writer,
        [&proof.combine([WemOriginField::Loop])?, frames],
    )?;
    proof.field(LOOP_POINTS, loop_points)?;
    if has_loop {
        for (path, field) in LOOP_FIELDS {
            let evidence = join(proof.writer, [&proof.combine([field])?, frames])?;
            proof.field(&format!("{LOOP_POINTS}/{path}"), evidence)?;
        }
    }
    proof.artifact(artifact)
}

pub(super) fn join<'a>(
    writer: &MediaWriter<'_>,
    parts: impl IntoIterator<Item = &'a OriginContributions>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    for part in parts {
        capture.include(part, writer)?;
    }
    Ok(capture.finish())
}
