// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemInspection, WemMetadata, WemOriginField, WemOriginPlan, WemPcmWav, WemPcmWriteError,
};
use p4kx_extraction::{AudioError, Codec, DecimalU64, WavReport};

use super::WemProjection;
use crate::audio_payload::{
    AudioPayload, ConversionFailure, ConversionOutcome, EncodedAudioFormat, WemConversion, WemLoop,
    WrittenAudio, WrittenSignal,
};
use crate::audio_record_origins as origins;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    Artifact, ArtifactExtension, MediaError, MediaIssue, MediaWriter, Severity, Status, error,
};

#[path = "audio_wem_conversion_evidence.rs"]
mod evidence;
#[path = "audio_wem_vorbis.rs"]
mod vorbis;

use self::evidence::{ArtifactEvidence, capture_fields};

pub(crate) const CONVERSION_NAMESPACE: &str = "audio-wem-conversion";

const OUTCOME: &str = "/data/payload/outcome";
const ARTIFACT: &str = "/data/payload/outcome/payload/artifact";
const SOURCE_RANGE: &str = "/data/payload/outcome/payload/source_range";
const LOOP_POINTS: &str = "/data/payload/outcome/payload/loop_points";

const LOOP_FIELDS: [(&str, WemOriginField); 3] = [
    ("start", WemOriginField::LoopStart),
    ("end", WemOriginField::LoopEnd),
    ("frames", WemOriginField::LoopFrames),
];

pub(super) fn project<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    format: EncodedAudioFormat,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match format {
        EncodedAudioFormat::Wav => {
            let converted = match inspected.wav() {
                Ok(conversion) => Ok(Completed::write(projection, conversion, writer)?),
                Err(cause) => Err(cause),
            };
            publish(projection, inspected.metadata(), format, converted, writer)
        }
        EncodedAudioFormat::Ogg => {
            let converted = vorbis::convert(projection, inspected, writer)?;
            publish(projection, inspected.metadata(), format, converted, writer)
        }
    }
}

fn publish(
    projection: &WemProjection<'_>,
    metadata: &WemMetadata,
    format: EncodedAudioFormat,
    converted: Result<impl CompletedConversion, AudioError>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut proof = ConversionOrigins::new(projection, metadata, writer)?;
    let (outcome, status, issue) = match converted {
        Ok(completed) => {
            let written = completed.into_record(&mut proof, projection)?;
            (ConversionOutcome::Written(written), Status::Complete, None)
        }
        Err(cause) => {
            proof.failure(&cause)?;
            let (code, label) = match format {
                EncodedAudioFormat::Wav => ("audio.wem.wav-conversion", "wav"),
                EncodedAudioFormat::Ogg => ("audio.wem.ogg-conversion", "ogg"),
            };
            let issue = MediaIssue::new(code, label, Severity::Error, cause.to_string());
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
            requested_format: format,
            source_codec: metadata.codec().id(),
            outcome,
        }),
        issue.as_slice(),
    )
}

trait CompletedConversion {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError>;
}

impl CompletedConversion for Completed {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError> {
        proof.success(&self)?;
        self.payload(projection)
    }
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

    fn payload(self, projection: &WemProjection<'_>) -> Result<WrittenAudio, MediaError> {
        Ok(WrittenAudio::Wav {
            signal: WrittenSignal {
                actual_codec: Codec::Pcm.id(),
                artifact: self.artifact,
                channels: self.report.channels,
                sample_rate: self.report.sample_rate,
                sample_count: DecimalU64::new(self.report.sample_count),
                loop_points: self
                    .report
                    .loop_points
                    .map(|points| WemLoop::from_frames(points.start, points.end)),
            },
            source_range: projection.range(self.source.start, self.source.end)?,
        })
    }
}

fn failure(cause: &AudioError) -> ConversionFailure {
    match cause {
        AudioError::UnsupportedCodec(_) | AudioError::UnsupportedOggCodec(_) => {
            ConversionFailure::UnsupportedCodec
        }
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

    fn common_success(&mut self) -> Result<(), MediaError> {
        let codec = self.combine([WemOriginField::Codec])?;
        self.field(OUTCOME, codec.clone())?;
        self.field(&format!("{OUTCOME}/type"), codec.clone())?;
        self.field(&format!("{OUTCOME}/payload/actual_format"), codec.clone())?;
        self.field(&format!("{OUTCOME}/payload/actual_codec"), codec)?;
        for (field, origin) in [
            ("channels", WemOriginField::Channels),
            ("sample_rate", WemOriginField::SampleRate),
        ] {
            self.field(
                &format!("{OUTCOME}/payload/{field}"),
                self.combine([origin])?,
            )?;
        }
        Ok(())
    }

    fn success(&mut self, completed: &Completed) -> Result<(), MediaError> {
        self.common_success()?;
        for (field, origin) in [
            ("sample_count", WemOriginField::SampleCount),
            ("loop_points", WemOriginField::Loop),
        ] {
            self.field(
                &format!("{OUTCOME}/payload/{field}"),
                self.combine([origin])?,
            )?;
        }
        if completed.report.loop_points.is_some() {
            for (field, origin) in LOOP_FIELDS {
                self.field(&format!("{LOOP_POINTS}/{field}"), self.combine([origin])?)?;
            }
        }
        let position = self.combine([WemOriginField::SampleCount])?;
        self.range_at(SOURCE_RANGE, &position)?;
        self.artifact(&completed.evidence)
    }

    fn range_at(&mut self, path: &str, position: &OriginContributions) -> Result<(), MediaError> {
        origins::range(&mut self.prepared, path, position, position, self.writer)?;
        self.subtree(
            &format!("{path}/basis"),
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
            AudioError::UnsupportedCodec(_) | AudioError::UnsupportedOggCodec(_) => {
                &[WemOriginField::Codec]
            }
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
        let mut observed = self.combine(fields.iter().copied())?;
        if matches!(
            cause,
            AudioError::InvalidVorbis(_) | AudioError::CodebooksRejected { .. }
        ) {
            observed.complete = false;
        }
        self.subtree(OUTCOME, observed.clone())?;
        self.subtree("/issues", observed)
    }
}
