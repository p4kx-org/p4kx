// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkDirectory, ChunkDirectoryError, ChunkError, Container};
use p4kx_extraction::model::PositionedSkeleton;

use super::character_payload::AttachmentRigDisposition;
use super::origins::{self, ModelOrigins};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{ContainerIdentity, Identity, MediaError, MediaWriter, RequiredInput, error};

const HEADER_BYTES: u64 = 16;

pub(super) enum RigRead {
    Unavailable {
        disposition: AttachmentRigDisposition,
        skeleton_chunk: Option<usize>,
        evidence: OriginContributions,
    },
    Ready(Box<VerifiedRig>),
}

pub(super) struct VerifiedRig {
    pub(super) skeleton: PositionedSkeleton,
    pub(super) input: MediaOriginInput,
    pub(super) ordinal: usize,
    pub(super) evidence: OriginContributions,
    pub(super) names: OriginContributions,
    pub(super) repaired_names: Vec<usize>,
}

pub(super) fn read(
    reader: &mut RequiredInput<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<RigRead, MediaError> {
    let entry = physical_entry(reader)?.entry_index;
    let input = reader.origin_input(writer)?;
    match reader.read_chunk_directory() {
        Ok(directory) => inspect(&directory, reader, input, entry, writer),
        Err(cause) => {
            let origins = ModelOrigins {
                input: &input,
                entry,
            };
            unreadable_directory(cause, reader, &origins, writer)
        }
    }
}

fn physical_entry<'a>(reader: &'a RequiredInput<'_>) -> Result<&'a ContainerIdentity, MediaError> {
    match &reader.receipt().source.identity {
        Identity::Entry { entry, .. } => Ok(entry),
        _ => Err(MediaError::Contract(
            "CDF rig input has no physical Entry".into(),
        )),
    }
}

fn unreadable_directory(
    cause: ChunkDirectoryError,
    reader: &RequiredInput<'_>,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<RigRead, MediaError> {
    let mut invalid_row = None;
    let disposition = match cause {
        ChunkDirectoryError::Format(ChunkError::Allocation) => {
            return Err(MediaError::Contract(
                "cannot allocate the CDF rig chunk directory".into(),
            ));
        }
        ChunkDirectoryError::Format(
            ChunkError::UnknownSignature { .. } | ChunkError::UnsupportedVersion { .. },
        ) => AttachmentRigDisposition::UnsupportedRig,
        ChunkDirectoryError::Format(_) => AttachmentRigDisposition::MalformedRig,
        ChunkDirectoryError::InvalidRow { range, .. } => {
            invalid_row = Some(range);
            AttachmentRigDisposition::MalformedRig
        }
        ChunkDirectoryError::Input(cause) => {
            return Err(error::io(Path::new(&physical_entry(reader)?.path), cause));
        }
        _ => {
            return Err(MediaError::Contract(
                "CDF rig chunk directory failed without a supported diagnostic".into(),
            ));
        }
    };
    let header_end = reader.receipt().source.size.min(HEADER_BYTES);
    let mut evidence = writer.capture_origins();
    evidence.include_original(&header_evidence(header_end, origins, writer)?)?;
    if let Some(range) = invalid_row {
        evidence.include_original(&origins.payload(
            range,
            "modelCdfRigInvalidRow",
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    Ok(unavailable(disposition, None, evidence.finish()))
}

fn inspect(
    directory: &ChunkDirectory,
    reader: &mut RequiredInput<'_>,
    input: MediaOriginInput,
    entry: usize,
    writer: &MediaWriter<'_>,
) -> Result<RigRead, MediaError> {
    let origins = ModelOrigins {
        input: &input,
        entry,
    };
    let directory_use = directory_evidence(directory, &origins, writer)?;
    if directory.container() != Container::Ivo {
        return Ok(unavailable(
            AttachmentRigDisposition::UnsupportedRig,
            None,
            directory_use,
        ));
    }
    let Some(ordinal) = PositionedSkeleton::primary_ordinal(directory.chunks()) else {
        return Ok(unavailable(
            AttachmentRigDisposition::NoSkeleton,
            None,
            directory_use,
        ));
    };
    let LocatedSkeleton { chunk, evidence } =
        located_skeleton(directory, ordinal, &directory_use, &origins, writer)?;
    if !PositionedSkeleton::supports(chunk) {
        return Ok(unavailable(
            AttachmentRigDisposition::UnsupportedRig,
            Some(ordinal),
            evidence,
        ));
    }
    let start = chunk.offset();
    let extent = start..start + chunk.size();
    let bytes = reader.read_chunk(extent.clone())?;
    let Ok(Some(skeleton)) = PositionedSkeleton::read(chunk, ordinal, &bytes) else {
        let mut invalid = writer.capture_origins();
        invalid.include_original(&evidence)?;
        invalid.include_original(&origins.payload(
            extent,
            "modelCdfRigMalformedSkeleton",
            SourceUseRole::Selector,
            writer,
        )?)?;
        return Ok(unavailable(
            AttachmentRigDisposition::MalformedRig,
            Some(ordinal),
            invalid.finish(),
        ));
    };
    reader.charge_metadata_work(skeleton.bones().len() as u64)?;
    let names = capture_names(&skeleton, &bytes, start, &origins, writer)?;
    Ok(RigRead::Ready(Box::new(VerifiedRig {
        skeleton,
        input,
        ordinal,
        evidence,
        names: names.evidence,
        repaired_names: names.repaired,
    })))
}

fn header_evidence(
    end: u64,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    origins.payload(0..end, "modelCdfRigHeader", SourceUseRole::Selector, writer)
}

fn directory_evidence(
    directory: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut evidence = writer.capture_origins();
    evidence.include_original(&header_evidence(HEADER_BYTES, origins, writer)?)?;
    evidence.include_original(&origins.payload(
        directory.table_range(),
        "modelCdfRigDirectory",
        SourceUseRole::Selector,
        writer,
    )?)?;
    Ok(evidence.finish())
}

struct LocatedSkeleton<'a> {
    chunk: &'a p4kx_extraction::chunks::Chunk,
    evidence: OriginContributions,
}

fn located_skeleton<'a>(
    directory: &'a ChunkDirectory,
    ordinal: usize,
    directory_use: &OriginContributions,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<LocatedSkeleton<'a>, MediaError> {
    let chunk = directory.chunks().get(ordinal).ok_or_else(inconsistent)?;
    let row = directory.table_row(ordinal).ok_or_else(inconsistent)?;
    let mut evidence = writer.capture_origins();
    evidence.include_original(directory_use)?;
    for (range, label) in [
        (row.kind, "modelCdfRigSkeletonType"),
        (row.version, "modelCdfRigSkeletonVersion"),
    ] {
        evidence.include_original(&origins.payload(
            range,
            label,
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    let extent = origins::chunk_extent(
        directory.table_range(),
        row.offset,
        row.size,
        origins,
        writer,
    )?;
    evidence.include_original(&extent.end)?;
    Ok(LocatedSkeleton {
        chunk,
        evidence: evidence.finish(),
    })
}

struct BoneNames {
    evidence: OriginContributions,
    repaired: Vec<usize>,
}

fn capture_names(
    skeleton: &PositionedSkeleton,
    bytes: &[u8],
    base: u64,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<BoneNames, MediaError> {
    let sources = skeleton.sources();
    let mut evidence = writer.capture_origins();
    evidence.include_original(&origins.payload(
        sources.count_range(),
        "modelCdfRigBoneCount",
        SourceUseRole::Selector,
        writer,
    )?)?;
    let mut table: Option<Range<u64>> = None;
    let mut repaired = Vec::new();
    for ordinal in 0..skeleton.bones().len() {
        writer.checkpoint()?;
        let name = sources.bone(ordinal).ok_or_else(inconsistent)?.name();
        let [Some(start), Some(end)] = [name.start, name.end].map(|at| {
            at.checked_sub(base)
                .and_then(|local| usize::try_from(local).ok())
        }) else {
            return Err(inconsistent());
        };
        let original = bytes.get(start..end).ok_or_else(inconsistent)?;
        if std::str::from_utf8(original).is_err() {
            repaired.push(ordinal);
        }
        if let Some(table) = &mut table {
            if table.end != name.start {
                return Err(inconsistent());
            }
            table.end = name.end;
        } else {
            table = Some(name);
        }
    }
    if let Some(table) = table {
        evidence.include_original(&origins.payload(
            table,
            "modelCdfRigBoneNames",
            SourceUseRole::Selector,
            writer,
        )?)?;
    }
    Ok(BoneNames {
        evidence: evidence.finish(),
        repaired,
    })
}

const fn unavailable(
    disposition: AttachmentRigDisposition,
    skeleton_chunk: Option<usize>,
    evidence: OriginContributions,
) -> RigRead {
    RigRead::Unavailable {
        disposition,
        skeleton_chunk,
        evidence,
    }
}

fn inconsistent() -> MediaError {
    MediaError::Contract("CDF rig lost an original skeleton location".into())
}
