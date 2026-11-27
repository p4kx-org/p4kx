// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::SkeletonSource;
use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use super::binding_payload::{Skeleton, SkeletonDisposition};
use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationPayload};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::reference_resolve::ReferenceObservations;
use crate::source_selection::EntrySelection;
use crate::{
    MediaError, MediaIssue, MediaWriter, OriginalIdentifier, Reference, ReferenceResolution,
    RequiredInput, Severity, SourceContext, Status, Subresource, origin_error,
};

pub(crate) struct SkeletonSelection {
    reference: Reference,
    candidates: ReferenceObservations<MediaOriginInput>,
    inventory: OriginContributions,
    read: SkeletonRead,
}

enum SkeletonRead {
    Ready(SelectedSkeleton),
    ReadWithoutSkeleton {
        input: MediaOriginInput,
        range: Range<u64>,
        state: SkeletonDisposition,
        failure: Option<String>,
    },
    Unread(SkeletonDisposition),
}

pub(super) struct SelectedSkeleton {
    pub(super) source: SkeletonSource,
    pub(super) input: MediaOriginInput,
    pub(super) table: Range<u64>,
}

pub(crate) fn validate_skeleton_path(path: &str) -> Result<(), MediaError> {
    if path.contains([':', '!', '\0'])
        || path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(MediaError::Contract(
            "animation skeleton must be one exact relative Entry path in the owning Archive".into(),
        ));
    }
    Ok(())
}

pub(super) fn selected<'a>(
    context: &'a SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<&'a SkeletonSelection>, MediaError> {
    let Some(path) = writer.animation_skeleton.as_deref() else {
        return Ok(None);
    };
    let cell = &context.directory.input_state()?.animation_skeleton;
    if cell.get().is_none() {
        let path = path.to_owned();
        let selection = lookup(context, &path, writer)?;
        cell.set(selection)
            .map_err(|_| MediaError::Contract("animation skeleton already resolved".into()))?;
    }
    Ok(cell.get())
}

fn lookup(
    context: &SourceContext<'_>,
    path: &str,
    writer: &mut MediaWriter<'_>,
) -> Result<SkeletonSelection, MediaError> {
    let observed = context.directory.lookup_input_observed(
        OriginalIdentifier::new_with_path(path),
        path,
        None,
        writer,
    )?;
    let read = if let Some(input) = observed.lookup.input {
        let entry = context
            .directory
            .index
            .archive
            .entries()
            .get(input.position)
            .ok_or_else(|| {
                MediaError::Contract("skeleton lookup lost its physical Entry".into())
            })?;
        if context
            .directory
            .selection
            .classify(context.directory.chain, entry.path())
            == EntrySelection::Excluded
        {
            SkeletonRead::Unread(SkeletonDisposition::ExplicitlyExcluded)
        } else {
            context.with_required_input(
                &input,
                &Subresource::new(wire::SKELETON, Vec::new()),
                writer,
                parse,
            )?
        }
    } else {
        SkeletonRead::Unread(match observed.lookup.reference.resolution() {
            ReferenceResolution::Missing => SkeletonDisposition::Missing,
            ReferenceResolution::Ambiguous { .. } => SkeletonDisposition::Ambiguous,
            _ => SkeletonDisposition::Unavailable,
        })
    };
    Ok(SkeletonSelection {
        reference: observed.lookup.reference,
        candidates: observed.evidence,
        inventory: observed.inventory,
        read,
    })
}

fn parse(
    required: &mut RequiredInput<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<SkeletonRead, MediaError> {
    let input = required.origin_input(writer)?;
    let bytes = required.read_payload()?;
    let extent = 0..bytes.len() as u64;
    let file = match ChunkFile::parse(bytes) {
        Ok(file) => file,
        Err(error) => {
            return Ok(SkeletonRead::ReadWithoutSkeleton {
                input,
                range: extent,
                state: SkeletonDisposition::InvalidContainer,
                failure: Some(error.to_string()),
            });
        }
    };
    let table = file.table_range();
    match SkeletonSource::read("animation skeleton", &file) {
        Ok(Some(source)) => Ok(SkeletonRead::Ready(SelectedSkeleton {
            source,
            input,
            table,
        })),
        Ok(None) => Ok(SkeletonRead::ReadWithoutSkeleton {
            input,
            range: table,
            state: SkeletonDisposition::NoSupportedSkeleton,
            failure: None,
        }),
        Err(error) => Ok(SkeletonRead::ReadWithoutSkeleton {
            input,
            range: extent,
            state: SkeletonDisposition::InvalidSkeleton,
            failure: Some(error.to_string()),
        }),
    }
}

impl SkeletonSelection {
    pub(super) const fn parsed(&self) -> Option<&SelectedSkeleton> {
        match &self.read {
            SkeletonRead::Ready(skeleton) => Some(skeleton),
            _ => None,
        }
    }

    pub(super) fn write(
        &self,
        origins: &AnimationOrigins<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = origins.prepared()?;
        let independent = origins.independent();
        let reference = ReferenceOriginInputs {
            original: &independent,
            selector: &independent,
            inventory: &self.inventory,
            semantic: &independent,
            candidates: &self.candidates,
            read_selected: false,
            identifier: ReferenceIdentifier::Path,
            subresource: ReferenceSubresource::None,
        };
        let path = FieldPath::parse("/data/payload/reference")
            .map_err(|cause| origin_error::address(&cause))?;
        reference.write(&mut prepared, &path, &self.reference, writer)?;
        let (state, evidence) = self.disposition(&reference, &mut prepared, writer)?;
        for field in ["/data/payload/skeleton", "/data/payload/skeleton/state"] {
            proof::at(&mut prepared, field, evidence.clone(), writer)?;
        }
        let issue = match &self.read {
            SkeletonRead::Unread(SkeletonDisposition::Unavailable) => {
                Some("the selected skeleton inventory is incomplete")
            }
            SkeletonRead::ReadWithoutSkeleton { failure, .. } => failure.as_deref(),
            _ => None,
        };
        let (status, issues) = if let Some(issue) = issue {
            proof::at(&mut prepared, "/issues", evidence, writer)?;
            (
                Status::Partial,
                vec![MediaIssue::new(
                    "animation.skeleton",
                    "skeleton",
                    Severity::Warning,
                    issue,
                )],
            )
        } else {
            (Status::Complete, Vec::new())
        };
        prepared.write(
            writer,
            &Subresource::new(wire::SKELETON, Vec::new()),
            status,
            &AnimationPayload::Skeleton(Skeleton {
                reference: &self.reference,
                skeleton: state,
            }),
            &issues,
        )
    }
    fn disposition(
        &self,
        reference: &ReferenceOriginInputs<'_, MediaOriginInput>,
        prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(SkeletonDisposition, OriginContributions), MediaError> {
        let mut disposition = writer.capture_origins();
        disposition.include(&reference.resolution(writer)?, writer)?;
        let state = match &self.read {
            SkeletonRead::Unread(state) => *state,
            SkeletonRead::ReadWithoutSkeleton {
                input,
                range,
                state,
                ..
            } => {
                disposition.include(
                    &bytes(input, range.clone(), "skeletonDisposition", writer)?,
                    writer,
                )?;
                *state
            }
            SkeletonRead::Ready(skeleton) => {
                let table = bytes(
                    &skeleton.input,
                    skeleton.table.clone(),
                    "skeletonChunkSelection",
                    writer,
                )?;
                disposition.include(&table, writer)?;
                proof::at(
                    prepared,
                    "/data/payload/skeleton/chunk_ordinal",
                    table,
                    writer,
                )?;
                proof::at(
                    prepared,
                    "/data/payload/skeleton/bone_count",
                    bytes(
                        &skeleton.input,
                        skeleton.source.bone_count_range(),
                        "boneCount",
                        writer,
                    )?,
                    writer,
                )?;
                SkeletonDisposition::Ready {
                    chunk_ordinal: skeleton.source.chunk_index(),
                    bone_count: skeleton.source.bone_count(),
                }
            }
        };
        Ok((state, disposition.finish()))
    }
}

pub(super) fn bytes(
    input: &MediaOriginInput,
    range: Range<u64>,
    field: &str,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new(
            "animation-skeleton-byte-range",
            &[range.start, range.end],
            field,
        ),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}
