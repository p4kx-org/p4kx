// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{Chunk, ChunkDirectory, ivo};
use p4kx_extraction::model::{ModelBoneSource, PositionedSkeleton};

use super::origins::{self, ModelOrigins};
use super::skeleton_record::{
    self as wire, SkeletonBone, SkeletonChunk, SkeletonRecord, SkeletonState,
};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(super) fn emit(
    file: &ChunkDirectory,
    ordinal: usize,
    reader: &mut RequiredReader<'_>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let chunk = file.chunks().get(ordinal).ok_or_else(missing)?;
    if !matches!(
        chunk.chunk_type(),
        ivo::COMPILED_BONES | ivo::COMPILED_BONES_2
    ) {
        return Ok(());
    }
    let row = file.table_row(ordinal).ok_or_else(missing)?;
    let mut selected = writer.capture_origins();
    for (range, name) in [
        (row.kind, "modelSkeletonChunkType"),
        (row.version, "modelSkeletonChunkVersion"),
    ] {
        selected.include_original(&origins.payload(
            range,
            name,
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    let selected = selected.finish();
    if !PositionedSkeleton::supports(chunk) {
        return write_chunk(
            ordinal,
            SkeletonState::UnsupportedVersion,
            None,
            &selected,
            None,
            origins,
            writer,
        );
    }
    read(chunk, ordinal, reader, &selected, origins, writer)
}

fn read(
    chunk: &Chunk,
    ordinal: usize,
    reader: &mut RequiredReader<'_>,
    selected: &OriginContributions,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let start = chunk.offset();
    let range = start..start + chunk.size();
    let bytes = reader.read_chunk(range.clone())?;
    let parsed = PositionedSkeleton::read(chunk, ordinal, &bytes);
    match parsed {
        Ok(Some(skeleton)) => {
            let mut selection = writer.capture_origins();
            selection.include_original(selected)?;
            selection.include_original(&origins.payload(
                skeleton.sources().count_range(),
                "modelSkeletonBoneCount",
                SourceUseRole::Payload,
                writer,
            )?)?;
            let count = selection.finish();
            write_chunk(
                ordinal,
                SkeletonState::Readable,
                Some(skeleton.bones().len()),
                &count,
                None,
                origins,
                writer,
            )?;
            write_bones(&skeleton, &bytes, start, &count, origins, writer)
        }
        Ok(None) => Err(missing()),
        Err(cause) => {
            let mut selection = writer.capture_origins();
            selection.include_original(selected)?;
            selection.include_original(&origins.payload(
                range,
                "modelSkeletonMalformed",
                SourceUseRole::Selector,
                writer,
            )?)?;
            write_chunk(
                ordinal,
                SkeletonState::Malformed,
                None,
                &selection.finish(),
                Some(cause.to_string()),
                origins,
                writer,
            )
        }
    }
}

fn write_chunk(
    ordinal: usize,
    disposition: SkeletonState,
    bone_count: Option<usize>,
    selection: &OriginContributions,
    issue: Option<String>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    for path in ["/subresource/indices", "/data/payload"] {
        origins::at(&mut prepared, path, selection.clone(), writer)?;
    }
    let (status, issues) = if let Some(issue) = issue {
        for path in ["/status", "/issues"] {
            origins::at(&mut prepared, path, selection.clone(), writer)?;
        }
        (
            Status::Partial,
            vec![MediaIssue::new(
                "model.skeleton",
                "chunk",
                Severity::Warning,
                issue,
            )],
        )
    } else {
        (Status::Complete, Vec::new())
    };
    prepared.write(
        writer,
        &Subresource::new(wire::CHUNK, vec![ordinal as u64]),
        status,
        &SkeletonRecord::SkeletonChunk(SkeletonChunk {
            disposition,
            bone_count,
        }),
        &issues,
    )
}

fn write_bones(
    skeleton: &PositionedSkeleton,
    bytes: &[u8],
    base: u64,
    selection: &OriginContributions,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let fields = skeleton.sources();
    for (ordinal, bone) in skeleton.bones().iter().enumerate() {
        writer.checkpoint()?;
        let field = fields.bone(ordinal).ok_or_else(missing)?;
        let mut prepared = origins.prepared()?;
        origins::at(
            &mut prepared,
            "/subresource/indices",
            selection.clone(),
            writer,
        )?;
        for (name, range) in [
            ("name", field.name()),
            ("name_lossy", field.name()),
            ("parent", field.parent()),
            ("controller_id", field.controller_id()),
        ] {
            bind(&mut prepared, name, range, origins, writer)?;
        }
        match field.node() {
            Some(range) => bind(&mut prepared, "node", range, origins, writer)?,
            None => origins::at(
                &mut prepared,
                "/data/payload/node",
                selection.clone(),
                writer,
            )?,
        }
        bind_poses(&mut prepared, field, origins, writer)?;
        let name = field.name();
        let local = usize::try_from(name.start - base)
            .ok()
            .zip(usize::try_from(name.end - base).ok());
        let raw_name = local
            .and_then(|(start, end)| bytes.get(start..end))
            .ok_or_else(missing)?;
        prepared.write(
            writer,
            &Subresource::new(wire::BONE, vec![fields.chunk() as u64, ordinal as u64]),
            Status::Complete,
            &SkeletonRecord::SkeletonBone(SkeletonBone {
                name: &bone.name,
                name_lossy: std::str::from_utf8(raw_name).is_err(),
                parent: bone.parent,
                controller_id: bone.controller_id,
                node: bone.node,
                relative: bone.relative.into(),
                model_space: bone.world.into(),
            }),
            &[],
        )?;
    }
    Ok(())
}

fn bind_poses(
    prepared: &mut PreparedRecordOrigins,
    field: &ModelBoneSource,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (key, span) in [
        ("relative", field.relative_pose()),
        ("model_space", field.world_pose()),
    ] {
        bind(prepared, key, span.clone(), origins, writer)?;
        for (name, start, end) in [
            ("rotation_xyzw", 0, 16),
            ("translation_z_up_metres", 16, 28),
        ] {
            bind(
                prepared,
                &format!("{key}/{name}"),
                span.start + start..span.start + end,
                origins,
                writer,
            )?;
        }
    }
    Ok(())
}

fn bind(
    prepared: &mut PreparedRecordOrigins,
    name: &str,
    range: Range<u64>,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let evidence = origins.payload(range, name, SourceUseRole::Payload, writer)?;
    origins::at(prepared, &format!("/data/payload/{name}"), evidence, writer)
}

fn missing() -> MediaError {
    MediaError::Contract("skeleton projection lost its original source coordinate".into())
}
