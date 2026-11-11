// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemFormatField, WemInspection, WemMetadata, WemOriginField, WemOriginPlan, WemPcmWav,
    WemPcmWriteError,
};
use p4kx_extraction::{AudioError, Codec, DecimalU64, SourceUseRole, WavReport};

use super::WemProjection;
use crate::audio_payload::{
    AudioPayload, ConversionFailure, ConversionOutcome, EncodedAudioFormat, WemConversion, WemLoop,
    WrittenWav,
};
use crate::audio_record_origins as origins;
use crate::audio_wem_origins;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    Artifact, ArtifactExtension, MediaError, MediaIssue, MediaWriter, Severity, Status, error,
};

pub(crate) const CONVERSION_NAMESPACE: &str = "audio-wem-conversion";

const OUTCOME: &str = "/data/payload/outcome";
const ARTIFACT: &str = "/data/payload/outcome/payload/artifact";
const SOURCE_RANGE: &str = "/data/payload/outcome/payload/source_range";
const LOOP_POINTS: &str = "/data/payload/outcome/payload/loop_points";

pub(super) fn wav<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let converted = match inspected.pcm_wav() {
        Ok(conversion) => Ok(Completed::write(projection, conversion, writer)?),
        Err(cause) => Err(cause),
    };
    let metadata = inspected.metadata();
    let mut proof = ConversionOrigins::new(projection, metadata, writer)?;
    let (outcome, status, issue) = match converted {
        Ok(completed) => {
            proof.success(&completed)?;
            (
                ConversionOutcome::Written(completed.payload(projection)?),
                Status::Complete,
                None,
            )
        }
        Err(cause) => {
            proof.failure(&cause)?;
            let issue = MediaIssue::new(
                "audio.wem.wav-conversion",
                "wav",
                Severity::Error,
                cause.to_string(),
            );
            (
                ConversionOutcome::Failed(failure(&cause)),
                Status::Partial,
                Some(issue),
            )
        }
    };
    proof.prepared.write(
        writer,
        &projection.subresource(CONVERSION_NAMESPACE, &[])?,
        status,
        &AudioPayload::WemConversion(WemConversion {
            requested_format: EncodedAudioFormat::Wav,
            source_codec: metadata.codec().id(),
            outcome,
        }),
        issue.as_slice(),
    )
}

struct Completed {
    artifact: Artifact,
    report: WavReport,
    source: Range<u64>,
    evidence: ArtifactEvidence,
}

impl Completed {
    fn write<R: Read + Seek>(
        projection: &WemProjection<'_>,
        conversion: WemPcmWav<'_, R>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let report = conversion.report();
        let source = conversion.source_range();
        let evidence =
            ArtifactEvidence::capture(projection, conversion.metadata(), &source, writer)?;
        let manifest = evidence.manifest(writer)?;
        let input = Path::new(projection.path);
        let artifact = writer.converted_artifact_with_origins(
            ArtifactExtension::new("wav")?,
            &manifest,
            |sink| {
                conversion.write(sink).map_err(|cause| match cause {
                    WemPcmWriteError::Input(cause) => error::io(input, cause),
                    WemPcmWriteError::Output(cause) => error::io(Path::new("WAV encoder"), cause),
                    other => error::io(input, io::Error::other(other)),
                })
            },
        )?;
        if artifact.size != report.size {
            return Err(MediaError::Contract(
                "WAV report and verified artifact sizes differ".into(),
            ));
        }
        Ok(Self {
            artifact,
            report,
            source,
            evidence,
        })
    }

    fn payload(self, projection: &WemProjection<'_>) -> Result<WrittenWav, MediaError> {
        Ok(WrittenWav {
            actual_format: EncodedAudioFormat::Wav,
            actual_codec: Codec::Pcm.id(),
            artifact: self.artifact,
            source_range: projection.range(self.source.start, self.source.end)?,
            channels: self.report.channels,
            sample_rate: self.report.sample_rate,
            sample_count: DecimalU64::new(self.report.sample_count),
            loop_points: self
                .report
                .loop_points
                .map(|points| WemLoop::from_frames(points.start, points.end)),
        })
    }
}

fn failure(cause: &AudioError) -> ConversionFailure {
    match cause {
        AudioError::UnsupportedCodec(_) => ConversionFailure::UnsupportedCodec,
        AudioError::WavTooLarge => ConversionFailure::WaveTooLarge,
        other => ConversionFailure::InvalidRepresentation {
            detail: other.to_string(),
        },
    }
}

struct ConversionOrigins<'source, 'info, 'writer, 'run> {
    projection: &'source WemProjection<'source>,
    plan: WemOriginPlan<'info>,
    writer: &'writer MediaWriter<'run>,
    prepared: PreparedRecordOrigins,
}

impl<'source, 'info, 'writer, 'run> ConversionOrigins<'source, 'info, 'writer, 'run> {
    fn new(
        projection: &'source WemProjection<'source>,
        metadata: &'info WemMetadata,
        writer: &'writer MediaWriter<'run>,
    ) -> Result<Self, MediaError> {
        let mut result = Self {
            projection,
            plan: WemOriginPlan::new(
                metadata.info_source(),
                metadata.codec(),
                metadata.sample_count(),
            ),
            writer,
            prepared: origins::prepare(projection.input)?,
        };
        let independent = OriginContributions::independent(&projection.input.owner);
        result.field("/data/payload/requested_format", independent.clone())?;
        let codec = result.combine([WemOriginField::Codec])?;
        result.field("/data/payload/source_codec", codec)?;
        projection.indices(&mut result.prepared, independent, writer)?;
        Ok(result)
    }

    fn combine(
        &self,
        fields: impl IntoIterator<Item = WemOriginField>,
    ) -> Result<OriginContributions, MediaError> {
        capture_fields(self.projection, &self.plan, fields, self.writer)
    }

    fn field(&mut self, path: &str, evidence: OriginContributions) -> Result<(), MediaError> {
        origins::field(&mut self.prepared, path, evidence, self.writer)
    }

    fn subtree(&mut self, path: &str, evidence: OriginContributions) -> Result<(), MediaError> {
        origins::subtree(&mut self.prepared, path, evidence, self.writer)
    }

    fn success(&mut self, completed: &Completed) -> Result<(), MediaError> {
        let codec = self.combine([WemOriginField::Codec])?;
        self.field(OUTCOME, codec.clone())?;
        self.field(&format!("{OUTCOME}/type"), codec.clone())?;
        self.field(&format!("{OUTCOME}/payload/actual_format"), codec.clone())?;
        self.field(&format!("{OUTCOME}/payload/actual_codec"), codec)?;
        for (field, origin) in [
            ("channels", WemOriginField::Channels),
            ("sample_rate", WemOriginField::SampleRate),
            ("sample_count", WemOriginField::SampleCount),
            ("loop_points", WemOriginField::Loop),
        ] {
            self.field(
                &format!("{OUTCOME}/payload/{field}"),
                self.combine([origin])?,
            )?;
        }
        if completed.report.loop_points.is_some() {
            for (field, origin) in [
                ("start", WemOriginField::LoopStart),
                ("end", WemOriginField::LoopEnd),
                ("frames", WemOriginField::LoopFrames),
            ] {
                self.field(&format!("{LOOP_POINTS}/{field}"), self.combine([origin])?)?;
            }
        }
        self.source_range()?;
        self.artifact(&completed.evidence)
    }

    fn source_range(&mut self) -> Result<(), MediaError> {
        let position = self.combine([WemOriginField::SampleCount])?;
        origins::range(
            &mut self.prepared,
            SOURCE_RANGE,
            &position,
            &position,
            self.writer,
        )?;
        self.subtree(
            &format!("{SOURCE_RANGE}/basis"),
            OriginContributions::independent(&self.projection.input.owner),
        )
    }

    fn artifact(&mut self, evidence: &ArtifactEvidence) -> Result<(), MediaError> {
        self.subtree(ARTIFACT, evidence.bytes.clone())?;
        self.field(&format!("{ARTIFACT}/size"), evidence.shape.clone())?;
        let independent = OriginContributions::independent(&self.projection.input.owner);
        self.field(&format!("{ARTIFACT}/digest/algorithm"), independent.clone())?;
        self.field(&format!("{ARTIFACT}/digest/scope"), independent)
    }

    fn failure(&mut self, cause: &AudioError) -> Result<(), MediaError> {
        let fields: &[WemOriginField] = match cause {
            AudioError::UnsupportedCodec(_) => &[WemOriginField::Codec],
            AudioError::WavTooLarge => &[
                WemOriginField::Channels,
                WemOriginField::SampleCount,
                WemOriginField::Loop,
            ],
            _ => &[
                WemOriginField::Channels,
                WemOriginField::SampleRate,
                WemOriginField::SampleCount,
            ],
        };
        let observed = self.combine(fields.iter().copied())?;
        self.subtree(OUTCOME, observed.clone())?;
        self.subtree("/issues", observed)
    }
}

struct ArtifactEvidence {
    bytes: OriginContributions,
    shape: OriginContributions,
}

impl ArtifactEvidence {
    fn capture(
        projection: &WemProjection<'_>,
        metadata: &WemMetadata,
        source: &Range<u64>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let plan = WemOriginPlan::new(
            metadata.info_source(),
            metadata.codec(),
            metadata.sample_count(),
        );
        let shape = capture_fields(
            projection,
            &plan,
            [
                WemOriginField::Channels,
                WemOriginField::SampleCount,
                WemOriginField::Loop,
            ],
            writer,
        )?;
        let channel_layout = (metadata.channels() > 2).then_some(WemOriginField::Format(
            WemFormatField::ChannelMask {
                layout_present: metadata.channel_layout().is_some(),
            },
        ));
        let header = capture_fields(
            projection,
            &plan,
            [WemOriginField::SampleRate, WemOriginField::RiffByteOrder]
                .into_iter()
                .chain(channel_layout),
            writer,
        )?;
        let mut capture = writer.capture_origins();
        capture.include(&shape, writer)?;
        capture.include(&header, writer)?;
        if !source.is_empty() {
            let indices = [source.start, source.end];
            let pcm = projection.capture(
                [(
                    OriginSite::new("wemPcmFrames", &indices, "originalSamples"),
                    SourceUseRole::Conversion,
                )],
                writer,
            )?;
            capture.include(&pcm, writer)?;
        }
        Ok(Self {
            bytes: capture.finish(),
            shape,
        })
    }

    fn manifest(&self, writer: &MediaWriter<'_>) -> Result<PreparedRecordOrigins, MediaError> {
        let mut manifest = writer.record_origins();
        let independent = OriginContributions::independent(&self.bytes.owner);
        for (field, evidence) in [
            ("/path", self.bytes.clone()),
            ("/size", self.shape.clone()),
            ("/digest/hex", self.bytes.clone()),
            ("/digest/algorithm", independent.clone()),
            ("/digest/scope", independent),
        ] {
            origins::field(&mut manifest, field, evidence, writer)?;
        }
        Ok(manifest)
    }
}

fn capture_fields(
    projection: &WemProjection<'_>,
    plan: &WemOriginPlan<'_>,
    fields: impl IntoIterator<Item = WemOriginField>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    for field in fields {
        let planned = audio_wem_origins::planned(projection, plan, field, writer)?;
        capture.include(&planned, writer)?;
    }
    Ok(capture.finish())
}
