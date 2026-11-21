// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemInspection, WemMetadata, WemOriginField, WemOriginPlan, WemReadError, WemVorbisOgg,
    WemVorbisRanges, WemVorbisWav, WemVorbisWriteError,
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
    EncodedAudioFormat, PcmInputs, VorbisInputs, VorbisWavInputs, WrittenAudio, WrittenOggFormat,
    WrittenWavFormat,
};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, error};

const INPUTS: &str = "/data/payload/outcome/payload/vorbis_inputs";

pub(super) struct CompletedVorbis {
    output: VorbisArtifact,
    ranges: WemVorbisRanges,
    origins: VorbisOrigins,
}

enum VorbisArtifact {
    Ogg(OggArtifact),
    Wav {
        output: WavArtifact,
        decoded: Range<u64>,
    },
}

enum PreparedVorbis<'source, R> {
    Ogg(WemVorbisOgg<'source, R>),
    Wav(WemVorbisWav<'source, R>),
}

impl<R> PreparedVorbis<'_, R> {
    const fn metadata(&self) -> &WemMetadata {
        match self {
            Self::Ogg(plan) => plan.metadata(),
            Self::Wav(plan) => plan.metadata(),
        }
    }

    const fn ranges(&self) -> &WemVorbisRanges {
        match self {
            Self::Ogg(plan) => plan.ranges(),
            Self::Wav(plan) => plan.ranges(),
        }
    }

    const fn decoded(&self) -> Option<&Range<u64>> {
        match self {
            Self::Ogg(_) => None,
            Self::Wav(plan) => Some(plan.decoded_audio_packets()),
        }
    }
}

impl<R: Read + Seek> PreparedVorbis<'_, R> {
    fn write(
        self,
        input: &Path,
        evidence: &ArtifactEvidence,
        writer: &mut MediaWriter<'_>,
    ) -> Result<VorbisArtifact, MediaError> {
        match self {
            Self::Ogg(plan) => {
                let output = ogg_artifact::write(
                    Codec::Vorbis,
                    plan.sample_count(),
                    evidence,
                    writer,
                    |sink| {
                        plan.write(sink)
                            .map_err(|cause| write_error(input, EncodedAudioFormat::Ogg, cause))
                    },
                )?;
                Ok(VorbisArtifact::Ogg(output))
            }
            Self::Wav(plan) => {
                let decoded = plan.decoded_audio_packets().clone();
                let output = wav_artifact::write(plan.report(), evidence, writer, |sink| {
                    plan.write(sink)
                        .map_err(|cause| write_error(input, EncodedAudioFormat::Wav, cause))
                })?;
                Ok(VorbisArtifact::Wav { output, decoded })
            }
        }
    }
}

pub(super) fn convert<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    format: EncodedAudioFormat,
    writer: &mut MediaWriter<'_>,
) -> Result<Result<CompletedVorbis, AudioError>, MediaError> {
    let input = Path::new(projection.path);
    let conversion = match format {
        EncodedAudioFormat::Ogg => inspected.vorbis_ogg().map(PreparedVorbis::Ogg),
        EncodedAudioFormat::Wav => inspected.vorbis_wav().map(PreparedVorbis::Wav),
    };
    let conversion = match conversion {
        Ok(conversion) => conversion,
        Err(WemReadError::Format(cause)) => return Ok(Err(cause)),
        Err(WemReadError::Io(cause)) => return Err(error::io(input, cause)),
        Err(cause) => return Err(MediaError::Contract(cause.to_string())),
    };
    let ranges = conversion.ranges().clone();
    let decoded = conversion.decoded();
    let origins =
        VorbisOrigins::capture(projection, conversion.metadata(), &ranges, decoded, writer)?;
    let result = recover_format_write(conversion.write(input, &origins.artifact, writer))?;
    Ok(result.map(|output| CompletedVorbis {
        output,
        ranges,
        origins,
    }))
}

fn write_error(input: &Path, output: EncodedAudioFormat, cause: WemVorbisWriteError) -> MediaError {
    match (output, cause) {
        (_, WemVorbisWriteError::Input(cause)) => error::io(input, cause),
        (EncodedAudioFormat::Wav, WemVorbisWriteError::Format(cause)) => {
            format_write_error(input, cause)
        }
        (EncodedAudioFormat::Wav, WemVorbisWriteError::Output(cause)) => {
            error::io(Path::new("WAV encoder"), cause)
        }
        (EncodedAudioFormat::Ogg, WemVorbisWriteError::Output(cause)) => {
            error::io(Path::new("Ogg encoder"), cause)
        }
        (_, cause) => MediaError::Contract(cause.to_string()),
    }
}

impl CompletedConversion for CompletedVorbis {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError> {
        let has_loop = match &self.output {
            VorbisArtifact::Ogg(output) => output.report.loop_points.is_some(),
            VorbisArtifact::Wav { output, .. } => output.report.loop_points.is_some(),
        };
        self.origins.prove(proof, has_loop)?;
        let range = |extent: Range<u64>| projection.range(extent.start, extent.end);
        let ranges = self.ranges;
        let setup = VorbisInputs {
            average_byte_rate: range(ranges.average_byte_rate)?,
            header: range(ranges.header)?,
            setup_packet: range(ranges.setup_packet)?,
        };
        Ok(match self.output {
            VorbisArtifact::Ogg(output) => WrittenAudio::Vorbis {
                actual_format: WrittenOggFormat::Ogg,
                signal: output.signal(),
                source_range: range(ranges.audio_packets)?,
                vorbis_inputs: Box::new(setup),
            },
            VorbisArtifact::Wav { output, decoded } => WrittenAudio::Pcm {
                actual_format: WrittenWavFormat::Wav,
                signal: output.signal(),
                source_range: range(decoded)?,
                inputs: PcmInputs::Vorbis {
                    vorbis_inputs: Box::new(VorbisWavInputs {
                        setup,
                        checked_audio_packets: range(ranges.audio_packets)?,
                    }),
                },
            },
        })
    }
}
struct VorbisOrigins {
    rate: OriginContributions,
    header: OriginContributions,
    setup: OriginContributions,
    audio: OriginContributions,
    checked: Option<OriginContributions>,
    frames: OriginContributions,
    artifact: ArtifactEvidence,
}

impl VorbisOrigins {
    fn capture(
        projection: &WemProjection<'_>,
        metadata: &WemMetadata,
        ranges: &WemVorbisRanges,
        decoded: Option<&Range<u64>>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let capture = |range, field| Self::range(projection, range, field, writer);
        let rate = capture(&ranges.average_byte_rate, "averageByteRate")?;
        let header = capture(&ranges.header, "configuration")?;
        let setup = capture(&ranges.setup_packet, "setupPacket")?;
        let audio_bytes = capture(&ranges.audio_packets, "audioPackets")?;
        let decoded = decoded
            .map(|range| capture(range, "decodedAudioPackets"))
            .transpose()?;
        let audio = join(writer, [decoded.as_ref().unwrap_or(&audio_bytes), &header])?;
        let plan = WemOriginPlan::new(
            metadata.info_source(),
            metadata.codec(),
            metadata.sample_count(),
        );
        let declared = capture_fields(projection, &plan, [WemOriginField::SampleCount], writer)?;
        let frames = join(writer, [&declared, &header, &setup, &audio_bytes])?;
        let fmt = capture_fields(
            projection,
            &plan,
            [
                WemOriginField::Channels,
                WemOriginField::SampleRate,
                WemOriginField::Loop,
                WemOriginField::RiffByteOrder,
            ],
            writer,
        )?;
        let bytes = join(writer, [&fmt, &frames, &rate])?;
        Ok(Self {
            rate,
            header,
            setup,
            audio,
            checked: decoded.map(|_| audio_bytes),
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
        prove_framed(
            proof,
            proof.combine([WemOriginField::Channels])?,
            &self.frames,
            &self.artifact,
            has_loop,
        )?;
        proof.range_at(SOURCE_RANGE, &self.audio)?;
        let fields = [
            ("average_byte_rate", &self.rate),
            ("header", &self.header),
            ("setup_packet", &self.setup),
        ];
        let fields = fields.into_iter().chain(
            self.checked
                .as_ref()
                .map(|origin| ("checked_audio_packets", origin)),
        );
        proof.field(
            INPUTS,
            join(proof.writer, fields.clone().map(|(_, origin)| origin))?,
        )?;
        for (field, origin) in fields {
            proof.range_at(&format!("{INPUTS}/{field}"), origin)?;
        }
        Ok(())
    }
    fn range(
        projection: &WemProjection<'_>,
        range: &Range<u64>,
        field: &'static str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let indices = [range.start, range.end];
        projection.capture(
            [(
                OriginSite::new("wemVorbisInput", &indices, field),
                SourceUseRole::Conversion,
            )],
            writer,
        )
    }
}
