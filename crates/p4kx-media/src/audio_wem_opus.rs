// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::sound::{
    WemInspection, WemMetadata, WemOpusRanges, WemOpusWriteError, WemOriginField, WemOriginPlan,
    WemReadError,
};
use p4kx_extraction::{AudioError, Codec, SourceUseRole};

use super::ogg_artifact::{self, OggArtifact, join};
use super::{
    ArtifactEvidence, CompletedConversion, ConversionOrigins, SOURCE_RANGE, WemProjection,
    capture_fields,
};
use crate::audio_payload::{OpusInputs, WrittenAudio, WrittenOggFormat};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, error};

const INPUTS: &str = "/data/payload/outcome/payload/opus_inputs";

pub(super) struct CompletedOpus {
    output: OggArtifact,
    ranges: WemOpusRanges,
    origins: OpusOrigins,
}

pub(super) fn convert<R: Read + Seek>(
    projection: &WemProjection<'_>,
    inspected: &mut WemInspection<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<Result<CompletedOpus, AudioError>, MediaError> {
    let input = Path::new(projection.path);
    let conversion = match inspected.opus_ogg() {
        Ok(conversion) => conversion,
        Err(WemReadError::Format(cause)) => return Ok(Err(cause)),
        Err(WemReadError::Io(cause)) => return Err(error::io(input, cause)),
        Err(cause) => return Err(MediaError::Contract(cause.to_string())),
    };
    let ranges = conversion.ranges().clone();
    let frames = conversion.sample_count();
    let origins = OpusOrigins::capture(projection, conversion.metadata(), &ranges, writer)?;
    let output = ogg_artifact::write(Codec::Opus, frames, &origins.artifact, writer, |sink| {
        conversion.write(sink).map_err(|cause| match cause {
            WemOpusWriteError::Input(WemReadError::Io(cause)) => error::io(input, cause),
            WemOpusWriteError::Output(cause) => error::io(Path::new("Ogg encoder"), cause),
            other => MediaError::Contract(other.to_string()),
        })
    })?;
    Ok(Ok(CompletedOpus {
        output,
        ranges,
        origins,
    }))
}

impl CompletedConversion for CompletedOpus {
    fn into_record(
        self,
        proof: &mut ConversionOrigins<'_, '_, '_, '_>,
        projection: &WemProjection<'_>,
    ) -> Result<WrittenAudio, MediaError> {
        self.origins
            .prove(proof, self.output.report.loop_points.is_some())?;
        let range = |range: Range<u64>| projection.range(range.start, range.end);
        let WemOpusRanges {
            format,
            seek_table,
            embedded_headers,
            checked_data,
            emitted_data,
            ..
        } = self.ranges;
        Ok(WrittenAudio::Opus {
            actual_format: WrittenOggFormat::Ogg,
            signal: self.output.signal(),
            source_range: range(emitted_data)?,
            opus_inputs: Box::new(OpusInputs {
                format: range(format)?,
                seek_table: seek_table.map(range).transpose()?,
                embedded_headers: embedded_headers.map(range).transpose()?,
                checked_data: range(checked_data)?,
            }),
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
        ogg_artifact::prove(proof, channels, &self.frames, &self.artifact, has_loop)?;
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
