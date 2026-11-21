// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::{
    AnimationChannel, AnimationController, CorruptAnimation, DecodedAnimationChannel,
};

use super::origin::{self as proof, AnimationOrigins};
use super::payload::{
    self as wire, AnimationPayload, Channel, Controller, DecodedSamples, Samples,
};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    ByteOrder, ChunkLayout, ChunkReference, DataType, MediaError, MediaIssue, MediaWriter,
    Severity, Status, Subresource,
};

pub(crate) fn write(
    clip: usize,
    controller: AnimationController<'_>,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.checkpoint()?;
    let mut prepared = controller_origins(&controller, origins, writer)?;
    let mut issues = Vec::new();
    let id = controller.id_range();
    let table = controller.table_range();
    let mut context = ChannelProjection {
        origins,
        prepared: &mut prepared,
        issues: &mut issues,
        writer,
    };
    let boundary = table.start + (table.end - table.start) / 2;
    let rotation = context.channel(
        "/data/payload/rotation",
        table.start..boundary,
        controller.rotation(),
        controller.decode_rotation(),
        "quaternions in CryEngine axes, x y z w, normalized with continuous sign",
    )?;
    let translation = context.channel(
        "/data/payload/translation",
        boundary..table.end,
        controller.translation(),
        controller.decode_translation(),
        "translations in CryEngine axes, x y z in metres",
    )?;
    let status = if issues.is_empty() {
        Status::Complete
    } else {
        Status::Partial
    };
    prepared.write(
        writer,
        &Subresource::new(
            wire::CONTROLLER,
            vec![clip as u64, controller.ordinal() as u64],
        ),
        status,
        &AnimationPayload::Controller(Controller {
            id: controller.controller_id(),
            id_range: wire::entry_range(id),
            table_range: wire::entry_range(table),
            rotation,
            translation,
        }),
        &issues,
    )
}

fn controller_origins(
    controller: &AnimationController<'_>,
    origins: &AnimationOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = origins.prepared()?;
    let id = controller.id_range();
    let table = controller.table_range();
    let evidence = origins.payload(id, "controllerId", SourceUseRole::Payload, writer)?;
    for field in ["/data/payload/controller_id", "/data/payload/id_range"] {
        proof::at(&mut prepared, field, evidence.clone(), writer)?;
    }
    let evidence = origins.payload(table, "controllerTableRow", SourceUseRole::Selector, writer)?;
    proof::control(&mut prepared, evidence.clone(), writer)?;
    proof::at(&mut prepared, "/data/payload/table_range", evidence, writer)?;
    for field in [
        "/data/payload/id_range/basis",
        "/data/payload/table_range/basis",
    ] {
        proof::at(&mut prepared, field, origins.independent(), writer)?;
    }
    Ok(prepared)
}

struct ChannelProjection<'a, 'source, 'writer> {
    origins: &'a AnimationOrigins<'source>,
    prepared: &'a mut PreparedRecordOrigins,
    issues: &'a mut Vec<MediaIssue>,
    writer: &'a mut MediaWriter<'writer>,
}

impl ChannelProjection<'_, '_, '_> {
    fn channel<const N: usize>(
        &mut self,
        prefix: &str,
        table: Range<u64>,
        declared: AnimationChannel,
        decoded: Result<Option<DecodedAnimationChannel<N>>, CorruptAnimation>,
        convention: &str,
    ) -> Result<Channel, MediaError> {
        self.descriptor(prefix, table.start)?;
        let sample_path = format!("{prefix}/samples");
        let samples = match decoded {
            Ok(Some(decoded)) => self.decoded(&sample_path, table, decoded, convention)?,
            Ok(None) => {
                let count = self.origins.payload(
                    table.start..table.start + 2,
                    "keyCount",
                    SourceUseRole::Selector,
                    self.writer,
                )?;
                proof::at(self.prepared, &sample_path, count, self.writer)?;
                Samples::Empty
            }
            Err(error) => {
                let unsupported = matches!(error, CorruptAnimation::UnknownFormat { .. });
                let (code, samples) = if unsupported {
                    ("animation.unsupported-channel", Samples::Unsupported)
                } else {
                    ("animation.malformed-channel", Samples::Malformed)
                };
                self.issues.push(MediaIssue::new(
                    code,
                    prefix,
                    Severity::Warning,
                    error.to_string(),
                ));
                proof::at(
                    self.prepared,
                    &sample_path,
                    self.origins.unknown(),
                    self.writer,
                )?;
                samples
            }
        };
        Ok(Channel {
            key_count: declared.key_count,
            packed_format: declared.packed_format,
            time_offset: declared.time_offset,
            value_offset: declared.value_offset,
            samples,
        })
    }

    fn descriptor(&mut self, prefix: &str, at: u64) -> Result<(), MediaError> {
        for (field, start, end) in [
            ("key_count", 0, 2),
            ("packed_format", 2, 4),
            ("time_offset", 4, 8),
            ("value_offset", 8, 12),
        ] {
            let evidence = self.origins.payload(
                at + start..at + end,
                field,
                SourceUseRole::Payload,
                self.writer,
            )?;
            proof::at(
                self.prepared,
                &format!("{prefix}/{field}"),
                evidence,
                self.writer,
            )?;
        }
        Ok(())
    }

    fn decoded<const N: usize>(
        &mut self,
        prefix: &str,
        table: Range<u64>,
        decoded: DecodedAnimationChannel<N>,
        convention: &str,
    ) -> Result<Samples, MediaError> {
        let mut evidence = self.writer.capture_origins();
        for (range, field) in [
            (decoded.frame_range.clone(), "frames"),
            (decoded.value_range.clone(), "values"),
        ] {
            evidence.include_original(&self.origins.decoded(
                table.clone(),
                range,
                field,
                self.writer,
            )?)?;
        }
        proof::at(
            self.prepared,
            &format!("{prefix}/layout"),
            evidence.finish(),
            self.writer,
        )?;
        let frames = self.chunk(
            &format!("{prefix}/frames"),
            table.clone(),
            decoded.frame_range,
            ChunkLayout::new(
                DataType::U16,
                ByteOrder::Little,
                decoded.keys.frames.len() as u64,
                "original frame indices",
            ),
            &decoded
                .keys
                .frames
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )?;
        let values = self.chunk(
            &format!("{prefix}/values"),
            table,
            decoded.value_range,
            ChunkLayout::new(
                DataType::F32,
                ByteOrder::Little,
                decoded.keys.values.len() as u64 * N as u64,
                convention,
            ),
            &decoded
                .keys
                .values
                .into_iter()
                .flatten()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )?;
        Ok(Samples::Decoded(Box::new(DecodedSamples {
            frames,
            values,
        })))
    }

    fn chunk(
        &mut self,
        prefix: &str,
        table: Range<u64>,
        source: Range<u64>,
        mut layout: ChunkLayout,
        mut bytes: &[u8],
    ) -> Result<ChunkReference, MediaError> {
        let evidence = self
            .origins
            .decoded(table, source.clone(), "sampleChunk", self.writer)?;
        proof::at(self.prepared, prefix, evidence.clone(), self.writer)?;
        for control in [
            "artifact/digest/algorithm",
            "artifact/digest/scope",
            "layout/dtype",
            "layout/byte_order",
            "layout/order",
            "layout/first",
            "layout/convention",
            "layout/source_range/basis",
        ] {
            proof::at(
                self.prepared,
                &format!("{prefix}/{control}"),
                self.origins.independent(),
                self.writer,
            )?;
        }
        layout.source_range = Some(wire::entry_range(source));
        let artifact = artifact_origins(self.origins, &evidence, self.writer)?;
        self.writer
            .chunk_with_origins(layout, &mut bytes, &artifact)
    }
}

fn artifact_origins(
    origins: &AnimationOrigins<'_>,
    evidence: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = PreparedRecordOrigins::new(&origins.input.owner);
    for field in ["/path", "/size", "/digest/hex"] {
        proof::at(&mut prepared, field, evidence.clone(), writer)?;
    }
    for field in ["/digest/algorithm", "/digest/scope"] {
        proof::at(&mut prepared, field, origins.independent(), writer)?;
    }
    Ok(prepared)
}
