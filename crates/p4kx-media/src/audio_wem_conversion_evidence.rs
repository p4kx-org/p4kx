// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::sound::{WemFormatField, WemMetadata, WemOriginField, WemOriginPlan};
use p4kx_extraction::{Codec, SourceUseRole};

use super::{ConversionOrigins, LOOP_FIELDS, LOOP_POINTS, OUTCOME, WemProjection};
use crate::audio_record_origins as origins;
use crate::audio_wem_origins;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct ArtifactEvidence {
    pub(super) bytes: OriginContributions,
    pub(super) shape: OriginContributions,
}

impl ArtifactEvidence {
    pub(super) fn capture(
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
            let (namespace, field) = match metadata.codec() {
                Codec::Pcm => ("wemPcmFrames", "originalSamples"),
                Codec::ImaAdpcm | Codec::PtAdpcm => ("wemAdpcmBlocks", "originalBlocks"),
                _ => {
                    return Err(MediaError::Contract(
                        "WAV preparation has no source encoding".into(),
                    ));
                }
            };
            let decoded = projection.capture(
                [(
                    OriginSite::new(namespace, &indices, field),
                    SourceUseRole::Conversion,
                )],
                writer,
            )?;
            capture.include(&decoded, writer)?;
        }
        Ok(Self {
            bytes: capture.finish(),
            shape,
        })
    }

    pub(super) fn manifest(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
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

pub(super) fn capture_fields(
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

pub(super) fn prove_framed(
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
