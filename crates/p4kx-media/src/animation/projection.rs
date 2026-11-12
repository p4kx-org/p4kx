// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ByteOrder as ChunkByteOrder, Chunk, ChunkFile, Container};
use p4kx_extraction::model::{AnimationMetadata, AnimationSource, AnimationTiming};

use super::binding_projection::ClipBinding;
use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationChunk, AnimationPayload, Clip, DatabaseName, Timing};
use super::skeleton_selection::SelectedSkeleton;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{ByteOrder, MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

type ParsedFile = ChunkFile<Vec<u8>>;

pub(crate) fn table(
    file: &ParsedFile,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (index, chunk) in file.chunks().iter().enumerate() {
        writer.checkpoint()?;
        chunk_row(file, index, chunk, origins, writer)?;
    }
    Ok(())
}

fn chunk_row(
    file: &ParsedFile,
    index: usize,
    chunk: &Chunk,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let row = file.table_row(index).ok_or_else(missing_row)?;
    let mut prepared = origins.prepared()?;
    let kind = origins.payload(row.kind, "chunkType", SourceUseRole::Payload, writer)?;
    for field in ["type_id", "type_name"] {
        put(&mut prepared, field, kind.clone(), writer)?;
    }
    proof::control(&mut prepared, kind, writer)?;
    let version = origins.payload(row.version, "chunkVersion", SourceUseRole::Payload, writer)?;
    let byte_order = match file.container() {
        Container::Ivo => {
            origins.payload(0..4, "containerSignature", SourceUseRole::Selector, writer)?
        }
        Container::CrCh => version.clone(),
    };
    put(&mut prepared, "version", version, writer)?;
    put(&mut prepared, "byte_order", byte_order, writer)?;
    let id = match row.id {
        Some(range) => origins.payload(range, "chunkId", SourceUseRole::Payload, writer)?,
        None => origins.payload(0..4, "containerSignature", SourceUseRole::Selector, writer)?,
    };
    put(&mut prepared, "chunk_id", id, writer)?;
    let range_inputs = file.table_range();
    let extent = origins.payload(
        range_inputs,
        "chunkExtentTable",
        SourceUseRole::Selector,
        writer,
    )?;
    put(&mut prepared, "range", extent, writer)?;
    put(&mut prepared, "range/basis", origins.independent(), writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::CHUNK, vec![index as u64]),
        Status::Complete,
        &AnimationPayload::Chunk(AnimationChunk {
            type_id: chunk.chunk_type().id(),
            type_name: chunk.chunk_type().name(),
            version: chunk.version(),
            chunk_id: chunk.id(),
            byte_order: match chunk.byte_order() {
                ChunkByteOrder::Little => ByteOrder::Little,
                ChunkByteOrder::Big => ByteOrder::Big,
            },
            range: wire::entry_range(chunk.offset()..chunk.offset() + chunk.size()),
        }),
        &[],
    )
}

pub(crate) fn clip(
    file: &ParsedFile,
    clip: AnimationSource<'_>,
    origins: &AnimationOrigins<'_>,
    skeleton: Option<&SelectedSkeleton>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.checkpoint()?;
    let mut prepared = origins.prepared()?;
    clip_metadata(file, &clip, origins, &mut prepared, writer)?;
    let timing = clip.metadata().timing();
    let name = clip.database_name();
    let mut issues = Vec::new();
    let timing = match timing {
        Ok(timing) => Some(timing_value(timing, &clip, origins, &mut prepared, writer)?),
        Err(error) => {
            let start = clip.metadata().range().start;
            let evidence = origins.payload(
                start + 4..start + 6,
                "frameRate",
                SourceUseRole::Selector,
                writer,
            )?;
            put(&mut prepared, "timing", evidence.clone(), writer)?;
            proof::at(&mut prepared, "/issues", evidence, writer)?;
            issues.push(MediaIssue::new(
                "animation.timing",
                "clip.timing",
                Severity::Warning,
                error.to_string(),
            ));
            None
        }
    };
    let metadata = clip.metadata();
    let status = if issues.is_empty() {
        Status::Complete
    } else {
        Status::Partial
    };
    prepared.write(
        writer,
        &Subresource::new(wire::CLIP, vec![clip.ordinal() as u64]),
        status,
        &AnimationPayload::Clip(Clip {
            metadata_chunk_version: metadata.version(),
            data_chunk_version: clip.data_chunk_version(),
            metadata_range: wire::entry_range(metadata.range()),
            metadata_bytes: metadata.bytes(),
            controller_block_range: wire::entry_range(clip.controller_block_range()),
            flags: metadata.flags(),
            timing,
            database_name: name.map(|name| DatabaseName {
                utf8: std::str::from_utf8(name.bytes()).ok(),
                range: wire::entry_range(name.range()),
            }),
        }),
        &issues,
    )?;
    controllers(&clip, origins, skeleton, writer)
}

fn controllers(
    clip: &AnimationSource<'_>,
    origins: &AnimationOrigins<'_>,
    skeleton: Option<&SelectedSkeleton>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let controllers = clip.controllers().map_err(|cause| {
        MediaError::Contract(format!("validated animation controller table: {cause}"))
    })?;
    let mut bindings =
        skeleton.map(|skeleton| ClipBinding::new(skeleton, clip.controller_ids_range()));
    for controller in controllers {
        super::controller_projection::write(clip.ordinal(), controller, origins, writer)?;
        if let Some(binding) = &mut bindings {
            binding.write(clip.ordinal(), &controller, origins, writer)?;
        }
    }
    Ok(())
}

fn clip_metadata(
    file: &ParsedFile,
    clip: &AnimationSource<'_>,
    origins: &AnimationOrigins<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let metadata = clip.metadata();
    let header = metadata.range();
    let metadata_row = file
        .table_row(metadata.chunk_index())
        .ok_or_else(missing_row)?;
    let data_row = file
        .table_row(clip.data_chunk_index())
        .ok_or_else(missing_row)?;
    metadata_bytes(metadata, origins, prepared, writer)?;
    put(
        prepared,
        "controller_block_range/basis",
        origins.independent(),
        writer,
    )?;
    if clip.database_name().is_some() {
        put(
            prepared,
            "database_name/range/basis",
            origins.independent(),
            writer,
        )?;
    }
    let block = clip.controller_block_range();
    for (field, range) in [
        ("metadata_chunk_version", metadata_row.version),
        ("data_chunk_version", data_row.version),
        ("flags", header.start..header.start + 4),
        ("controller_block_range", block),
    ] {
        let evidence = origins.payload(range, field, SourceUseRole::Payload, writer)?;
        put(prepared, field, evidence, writer)?;
    }
    let name = match clip.database_name() {
        Some(name) => {
            origins.payload(name.range(), "databaseName", SourceUseRole::Payload, writer)?
        }
        None => origins.extension(writer)?,
    };
    put(prepared, "database_name", name, writer)
}

fn metadata_bytes(
    metadata: AnimationMetadata<'_>,
    origins: &AnimationOrigins<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let header = metadata.range();
    let evidence = origins.payload(
        header.clone(),
        "clipMetadata",
        SourceUseRole::Payload,
        writer,
    )?;
    proof::control(prepared, evidence.clone(), writer)?;
    for field in ["metadata_range", "metadata_bytes"] {
        put(prepared, field, evidence.clone(), writer)?;
    }
    put(
        prepared,
        "metadata_range/basis",
        origins.independent(),
        writer,
    )?;
    for index in 0..metadata.bytes().len() {
        let at = header.start + index as u64;
        let evidence = origins.payload(
            at..at + 1,
            "clipMetadataByte",
            SourceUseRole::Payload,
            writer,
        )?;
        put(
            prepared,
            &format!("metadata_bytes/{index}"),
            evidence,
            writer,
        )?;
    }
    Ok(())
}

fn timing_value(
    timing: AnimationTiming,
    clip: &AnimationSource<'_>,
    origins: &AnimationOrigins<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<Timing, MediaError> {
    let start = clip.metadata().range().start;
    for (field, start_offset, end_offset) in [
        ("timing/additive", 0, 4),
        ("timing/frames_per_second", 4, 6),
        ("timing/end_frame", 16, 20),
    ] {
        let evidence = origins.payload(
            start + start_offset..start + end_offset,
            field,
            SourceUseRole::Payload,
            writer,
        )?;
        put(prepared, field, evidence, writer)?;
    }
    Ok(Timing {
        frames_per_second: timing.frames_per_second,
        end_frame: timing.end_frame,
        additive: timing.additive,
    })
}

fn put(
    prepared: &mut PreparedRecordOrigins,
    field: &str,
    evidence: crate::origin_contributions::OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    proof::at(
        prepared,
        &format!("/data/payload/{field}"),
        evidence,
        writer,
    )
}

fn missing_row() -> MediaError {
    MediaError::Contract("validated animation chunk table row is missing".into())
}
