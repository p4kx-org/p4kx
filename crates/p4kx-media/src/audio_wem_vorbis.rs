// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemInspection, WemMetadata, WemOriginField, WemOriginPlan, WemReadError, WemVorbisRanges,
    WemVorbisWriteError,
};
use p4kx_extraction::{AudioError, Codec, SourceUseRole};

use super::evidence::{join, prove_framed};
use super::ogg_artifact::{self, OggArtifact};
use super::{
    ArtifactEvidence, CompletedConversion, ConversionOrigins, SOURCE_RANGE, WemProjection,
    capture_fields,
};
use crate::audio_payload::{VorbisInputs, WrittenAudio, WrittenOggFormat};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, error};

const INPUTS: &str = "/data/payload/outcome/payload/vorbis_inputs";

pub(super) struct CompletedOgg {
    output: OggArtifact,
    ranges: WemVorbisRanges,
    origins: VorbisOrigins,
}

pub(super) fn convert<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<Result<CompletedOgg, AudioError>, MediaError> {
    let input = Path::new(projection.path);
    let conversion = match inspected.vorbis_ogg() {
        Ok(conversion) => conversion,
        Err(WemReadError::Format(cause)) => return Ok(Err(cause)),
        Err(WemReadError::Io(cause)) => return Err(error::io(input, cause)),
        Err(cause) => return Err(MediaError::Contract(cause.to_string())),
    };
    let ranges = conversion.ranges().clone();
    let frames = conversion.sample_count();
    let origins = VorbisOrigins::capture(projection, conversion.metadata(), &ranges, writer)?;
    let output = ogg_artifact::write(Codec::Vorbis, frames, &origins.artifact, writer, |sink| {
        conversion.write(sink).map_err(|cause| match cause {
            WemVorbisWriteError::Input(cause) => error::io(input, cause),
            WemVorbisWriteError::Output(cause) => error::io(Path::new("Ogg encoder"), cause),
            other => MediaError::Contract(other.to_string()),
        })
    })?;
    Ok(Ok(CompletedOgg {
        output,
        ranges,
        origins,
    }))
}

impl CompletedConversion for CompletedOgg {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError> {
        self.prove(proof)?;
        self.payload(projection)
    }
}

impl CompletedOgg {
    fn payload(self, projection: &WemProjection<'_>) -> Result<WrittenAudio, MediaError> {
        let ranges = self.ranges;
        Ok(WrittenAudio::Vorbis {
            actual_format: WrittenOggFormat::Ogg,
            signal: self.output.signal(),
            source_range: projection.range(ranges.audio_packets.start, ranges.audio_packets.end)?,
            vorbis_inputs: Box::new(VorbisInputs {
                average_byte_rate: projection
                    .range(ranges.average_byte_rate.start, ranges.average_byte_rate.end)?,
                header: projection.range(ranges.header.start, ranges.header.end)?,
                setup_packet: projection
                    .range(ranges.setup_packet.start, ranges.setup_packet.end)?,
            }),
        })
    }

    fn prove(&self, proof: &mut ConversionOrigins<'_, '_, '_, '_>) -> Result<(), MediaError> {
        prove_framed(
            proof,
            proof.combine([WemOriginField::Channels])?,
            &self.origins.frames,
            &self.origins.artifact,
            self.output.report.loop_points.is_some(),
        )?;
        proof.range_at(SOURCE_RANGE, &self.origins.audio)?;
        proof.field(
            INPUTS,
            join(
                proof.writer,
                [
                    &self.origins.rate,
                    &self.origins.header,
                    &self.origins.setup,
                ],
            )?,
        )?;
        for (field, origin) in [
            ("average_byte_rate", &self.origins.rate),
            ("header", &self.origins.header),
            ("setup_packet", &self.origins.setup),
        ] {
            proof.range_at(&format!("{INPUTS}/{field}"), origin)?;
        }
        Ok(())
    }
}

struct VorbisOrigins {
    rate: OriginContributions,
    header: OriginContributions,
    setup: OriginContributions,
    audio: OriginContributions,
    frames: OriginContributions,
    artifact: ArtifactEvidence,
}

impl VorbisOrigins {
    fn capture(
        projection: &WemProjection<'_>,
        metadata: &WemMetadata,
        ranges: &WemVorbisRanges,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let capture = |range, field| Self::range(projection, range, field, writer);
        let rate = capture(&ranges.average_byte_rate, "averageByteRate")?;
        let header = capture(&ranges.header, "configuration")?;
        let setup = capture(&ranges.setup_packet, "setupPacket")?;
        let audio_bytes = capture(&ranges.audio_packets, "audioPackets")?;
        let audio = join(writer, [&audio_bytes, &header])?;
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
            frames,
            artifact: ArtifactEvidence {
                shape: bytes.clone(),
                bytes,
            },
        })
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
