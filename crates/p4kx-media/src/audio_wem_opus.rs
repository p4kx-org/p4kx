// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemInspection, WemMetadata, WemOpusOgg, WemOpusRanges, WemOpusWav, WemOpusWriteError,
    WemOriginField, WemOriginPlan, WemReadError,
};
use p4kx_extraction::{AudioError, Codec, SourceUseRole};

use super::evidence::{join, prove_framed};
use super::ogg_artifact::{self, OggArtifact};
use super::wav_artifact::{self, WavArtifact};
use super::{
    ArtifactEvidence, CompletedConversion, ConversionOrigins, SOURCE_RANGE, WemProjection,
    capture_fields, format_write_error, recover_format_write,
};
use crate::audio_payload::{
    EncodedAudioFormat, OpusInputs, PcmInputs, WrittenAudio, WrittenOggFormat, WrittenWavFormat,
};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, error};

const INPUTS: &str = "/data/payload/outcome/payload/opus_inputs";

pub(super) struct CompletedOpus {
    output: OpusArtifact,
    ranges: WemOpusRanges,
    origins: OpusOrigins,
}

enum OpusArtifact {
    Ogg(OggArtifact),
    Wav(WavArtifact),
}

enum PreparedOpus<'source, R> {
    Ogg(WemOpusOgg<'source, R>),
    Wav(WemOpusWav<'source, R>),
}

impl<R> PreparedOpus<'_, R> {
    const fn metadata(&self) -> &WemMetadata {
        match self {
            Self::Ogg(plan) => plan.metadata(),
            Self::Wav(plan) => plan.metadata(),
        }
    }

    const fn ranges(&self) -> &WemOpusRanges {
        match self {
            Self::Ogg(plan) => plan.ranges(),
            Self::Wav(plan) => plan.ranges(),
        }
    }
}

impl<R: Read + Seek> PreparedOpus<'_, R> {
    fn write(
        self,
        input: &Path,
        evidence: &ArtifactEvidence,
        writer: &mut MediaWriter<'_>,
    ) -> Result<OpusArtifact, MediaError> {
        match self {
            Self::Ogg(plan) => {
                let output = ogg_artifact::write(
                    Codec::Opus,
                    plan.sample_count(),
                    evidence,
                    writer,
                    |sink| {
                        plan.write(sink)
                            .map_err(|cause| write_error(input, "Ogg encoder", cause))
                    },
                )?;
                Ok(OpusArtifact::Ogg(output))
            }
            Self::Wav(plan) => {
                let output = wav_artifact::write(plan.report(), evidence, writer, |sink| {
                    plan.write(sink)
                        .map_err(|cause| write_error(input, "WAV encoder", cause))
                })?;
                Ok(OpusArtifact::Wav(output))
            }
        }
    }
}

pub(super) fn convert<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    format: EncodedAudioFormat,
    writer: &mut MediaWriter<'_>,
) -> Result<Result<CompletedOpus, AudioError>, MediaError> {
    let input = Path::new(projection.path);
    let conversion = match format {
        EncodedAudioFormat::Ogg => inspected.opus_ogg().map(PreparedOpus::Ogg),
        EncodedAudioFormat::Wav => inspected.opus_wav().map(PreparedOpus::Wav),
    };
    let conversion = match conversion {
        Ok(conversion) => conversion,
        Err(WemReadError::Format(cause)) => return Ok(Err(cause)),
        Err(WemReadError::Io(cause)) => return Err(error::io(input, cause)),
        Err(cause) => return Err(MediaError::Contract(cause.to_string())),
    };
    let ranges = conversion.ranges().clone();
    let origins = OpusOrigins::capture(projection, conversion.metadata(), &ranges, writer)?;
    let result = recover_format_write(conversion.write(input, &origins.artifact, writer))?;
    Ok(result.map(|output| CompletedOpus {
        output,
        ranges,
        origins,
    }))
}

fn write_error(input: &Path, output: &str, cause: WemOpusWriteError) -> MediaError {
    match cause {
        WemOpusWriteError::Input(WemReadError::Io(cause)) => error::io(input, cause),
        WemOpusWriteError::Input(WemReadError::Format(cause)) => format_write_error(input, cause),
        WemOpusWriteError::Output(cause) => error::io(Path::new(output), cause),
        other => MediaError::Contract(other.to_string()),
    }
}

impl CompletedConversion for CompletedOpus {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError> {
        let has_loop = match &self.output {
            OpusArtifact::Ogg(output) => output.report.loop_points.is_some(),
            OpusArtifact::Wav(output) => output.report.loop_points.is_some(),
        };
        self.origins.prove(proof, has_loop)?;
        let range = |range: Range<u64>| projection.range(range.start, range.end);
        let WemOpusRanges {
            format,
            seek_table,
            embedded_headers,
            checked_data,
            emitted_data,
            ..
        } = self.ranges;
        let source_range = range(emitted_data)?;
        let inputs = Box::new(OpusInputs {
            format: range(format)?,
            seek_table: seek_table.map(range).transpose()?,
            embedded_headers: embedded_headers.map(range).transpose()?,
            checked_data: range(checked_data)?,
        });
        Ok(match self.output {
            OpusArtifact::Ogg(output) => WrittenAudio::Opus {
                actual_format: WrittenOggFormat::Ogg,
                signal: output.signal(),
                source_range,
                opus_inputs: inputs,
            },
            OpusArtifact::Wav(output) => WrittenAudio::Pcm {
                actual_format: WrittenWavFormat::Wav,
                signal: output.signal(),
                source_range,
                inputs: PcmInputs::Opus {
                    opus_inputs: inputs,
                },
            },
        })
    }
}

struct OpusOrigins {
    format: OriginContributions,
    seek: Option<OriginContributions>,
    headers: Option<OriginContributions>,
    checked: OriginContributions,
    emitted: OriginContributions,
    frames: OriginContributions,
    artifact: ArtifactEvidence,
}

impl OpusOrigins {
    fn capture(
        projection: &WemProjection<'_>,
        metadata: &WemMetadata,
        ranges: &WemOpusRanges,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let capture = |range: &Range<u64>, field| {
            let indices = [range.start, range.end];
            projection.capture(
                [(
                    OriginSite::new("wemOpusInput", &indices, field),
                    SourceUseRole::Conversion,
                )],
                writer,
            )
        };
        let format = capture(&ranges.format, "format")?;
        let seek = ranges
            .seek_table
            .as_ref()
            .map(|range| capture(range, "seekTable"))
            .transpose()?;
        let headers = ranges
            .embedded_headers
            .as_ref()
            .map(|range| capture(range, "headerPages"))
            .transpose()?;
        let checked = capture(&ranges.checked_data, "checkedPackets")?;
        let emitted = capture(&ranges.emitted_data, "emittedPackets")?;
        let plan = WemOriginPlan::new(
            metadata.info_source(),
            metadata.codec(),
            metadata.sample_count(),
        );
        let sample = capture_fields(projection, &plan, [WemOriginField::SampleCount], writer)?;
        let packet_inputs = [&format, &checked]
            .into_iter()
            .chain(seek.iter())
            .chain(headers.iter());
        let frames = join(writer, packet_inputs.chain([&sample]))?;
        let metadata_fields = capture_fields(
            projection,
            &plan,
            [
                WemOriginField::Channels,
                WemOriginField::Loop,
                WemOriginField::RiffByteOrder,
            ],
            writer,
        )?;
        let bytes = join(writer, [&metadata_fields, &frames, &emitted])?;
        Ok(Self {
            format,
            seek,
            headers,
            checked,
            emitted,
            frames,
            artifact: ArtifactEvidence {
                shape: bytes.clone(),
                bytes,
            },
        })
    }

    fn prove(
        &self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        has_loop: bool,
    ) -> Result<(), MediaError> {
        let declared_channels = proof.combine([WemOriginField::Channels])?;
        let channels = join(
            proof.writer,
            [&declared_channels, &self.format]
                .into_iter()
                .chain(self.headers.iter()),
        )?;
        prove_framed(proof, channels, &self.frames, &self.artifact, has_loop)?;
        let selected = join(proof.writer, [&self.emitted, &self.frames])?;
        proof.range_at(SOURCE_RANGE, &selected)?;
        let inputs = join(
            proof.writer,
            [&self.format, &self.checked]
                .into_iter()
                .chain(self.seek.iter())
                .chain(self.headers.iter()),
        )?;
        proof.field(INPUTS, inputs)?;
        proof.range_at(&format!("{INPUTS}/format"), &self.format)?;
        let checked = join(
            proof.writer,
            [&self.format, &self.checked]
                .into_iter()
                .chain(self.seek.iter()),
        )?;
        proof.range_at(&format!("{INPUTS}/checked_data"), &checked)?;
        self.optional_range(proof, "seek_table", self.seek.as_ref())?;
        self.optional_range(proof, "embedded_headers", self.headers.as_ref())
    }

    fn optional_range(
        &self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        field: &str,
        range: Option<&OriginContributions>,
    ) -> Result<(), MediaError> {
        let path = format!("{INPUTS}/{field}");
        if let Some(range) = range {
            let proof_of_range = join(proof.writer, [&self.format, range])?;
            proof.range_at(&path, &proof_of_range)
        } else {
            proof.field(&path, self.format.clone())
        }
    }
}
