// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{
    AnimationError, DbaAnimationSources, DbaMetadataRecord, DbaMetadataRecords,
};

use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationPayload, ControllerDisposition, DbaMetadata};
use super::projection;
use super::skeleton_selection::SelectedSkeleton;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

type ParsedFile = ChunkFile<Vec<u8>>;

pub(super) fn clips(
    name: &str,
    file: &ParsedFile,
    origins: &AnimationOrigins<'_>,
    skeleton: Option<&SelectedSkeleton>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<AnimationError>, MediaError> {
    let mut first_uncovered = 0;
    let failure = match DbaAnimationSources::read(name, file) {
        Ok(clips) => {
            let mut failure = None;
            for result in clips {
                match result {
                    Ok(clip) => {
                        first_uncovered = clip.ordinal() + 1;
                        projection::clip(file, clip, origins, skeleton, writer)?;
                    }
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            failure
        }
        Err(error) => Some(error),
    };
    if failure.is_some() {
        metadata_tail(name, file, first_uncovered, origins, writer)?;
    }
    Ok(failure)
}

fn metadata_tail(
    name: &str,
    file: &ParsedFile,
    first_uncovered: usize,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Ok(records) = DbaMetadataRecords::read(name, file) else {
        return Ok(());
    };
    for record in records {
        writer.checkpoint()?;
        let Ok(record) = record else {
            break;
        };
        if record.ordinal >= first_uncovered {
            metadata_row(file, record, origins, writer)?;
        }
    }
    Ok(())
}

fn metadata_row(
    file: &ParsedFile,
    record: DbaMetadataRecord<'_>,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    let (timing, mut issues) = projection::clip_metadata(
        file,
        record.metadata,
        Some(record.name),
        origins,
        &mut prepared,
        writer,
    )?;
    proof::at(
        &mut prepared,
        "/data/payload/controller_disposition",
        origins.unknown(),
        writer,
    )?;
    proof::at(
        &mut prepared,
        &format!("/issues/{}", issues.len()),
        origins.unknown(),
        writer,
    )?;
    issues.push(MediaIssue::new(
        "animation.controller-block-unavailable",
        "clip.controllers",
        Severity::Warning,
        "The controller block extent could not be established for this metadata record.",
    ));
    prepared.write(
        writer,
        &Subresource::new(wire::DBA_METADATA, vec![record.ordinal as u64]),
        Status::Partial,
        &AnimationPayload::DbaMetadata(DbaMetadata {
            metadata_chunk_version: record.metadata.version(),
            metadata_range: wire::entry_range(record.metadata.range()),
            metadata_bytes: record.metadata.bytes(),
            flags: record.metadata.flags(),
            timing,
            database_name: projection::database_name(record.name),
            controller_disposition: ControllerDisposition::Unavailable,
        }),
        &issues,
    )
}
