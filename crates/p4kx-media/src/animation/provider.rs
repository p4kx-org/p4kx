// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{AnimationError, AnimationSource};

use super::image_projection;
use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationPayload, Entry, EntryLayout, Family};
use super::skeleton_selection::{self, SelectedSkeleton};
use crate::{
    Capability, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter, Projection, Provider,
    Severity, SourceContext, Status, Subresource, required_reader,
};

pub(crate) struct AnimationProvider;

impl Provider for AnimationProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Animation,
            "p4kx.animation.source",
            "1.3.0",
            "Exact animation and image Entry bytes with positioned source data and separate decoded sample chunks.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "chunk-table",
                "Every original chunk type, version and source extent.",
            ),
            FieldGroup::new(
                "caf-and-dba-clips",
                "Metadata records, complete flags, original DBA name positions, controller encoding words, frames and decoded keys in authored order.",
            ),
            FieldGroup::new(
                "recoverable-dba-metadata",
                "Physical DBA records and names that do not have a readable controller block.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "aim-pose-semantics",
                "AIM retains original bytes and chunk table without interpreting poses.",
            ),
            FieldGroup::new(
                "animation-images",
                "Global animation image chunks and ordered CAF image paths.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "directional-image-records",
                "The directional image retains original bytes without interpreting record boundaries.",
            ),
            FieldGroup::new(
                "skeleton-binding",
                "Optional exact Entry in the owning Archive; original controller and bone selections are supplementary.",
            ),
        ];
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let name = context.entry.path();
        let Some(family) = Family::of(name) else {
            return Ok(Projection::Skipped);
        };
        let input = context.origin_input(writer)?;
        let origins = AnimationOrigins {
            input: &input,
            entry: context.entry.index(),
        };
        let bytes = required_reader::read_all(context, writer)?;
        let size = bytes.len() as u64;
        let file = match ChunkFile::parse(bytes) {
            Ok(file) => file,
            Err(error) => {
                entry(family, None, Some(&error), size, &origins, writer)?;
                return Ok(Projection::Written);
            }
        };
        super::projection::table(&file, &origins, writer)?;
        let skeleton = match family {
            Family::Caf | Family::Dba => skeleton_selection::selected(context, writer)?,
            Family::Aim | Family::CafImage | Family::DirectionalBlendImage => None,
        };
        if let Some(skeleton) = skeleton {
            skeleton.write(&origins, writer)?;
        }
        let skeleton = skeleton.and_then(skeleton_selection::SkeletonSelection::parsed);
        image_projection::write(&file, family, context, &origins, writer)?;
        let issue = clips(name, family, &file, &origins, skeleton, writer)?;
        entry(
            family,
            Some(file.chunks().len()),
            issue.as_ref().map(|cause| cause as &dyn Error),
            size,
            &origins,
            writer,
        )?;
        Ok(Projection::Written)
    }
}

fn clips(
    name: &str,
    family: Family,
    file: &ChunkFile<Vec<u8>>,
    origins: &AnimationOrigins<'_>,
    skeleton: Option<&SelectedSkeleton>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<AnimationError>, MediaError> {
    match family {
        Family::Aim | Family::CafImage | Family::DirectionalBlendImage => {}
        Family::Caf => match AnimationSource::read_caf(name, file) {
            Ok(clip) => super::projection::clip(file, clip, origins, skeleton, writer)?,
            Err(error) => return Ok(Some(error)),
        },
        Family::Dba => return super::dba_projection::clips(name, file, origins, skeleton, writer),
    }
    Ok(None)
}

fn entry(
    family: Family,
    chunk_count: Option<usize>,
    failure: Option<&dyn Error>,
    size: u64,
    origins: &AnimationOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    let extension = origins.extension(writer)?;
    proof::at(
        &mut prepared,
        "/data/payload/family",
        extension.clone(),
        writer,
    )?;
    let mut issues = Vec::new();
    let status = if let Some(error) = failure {
        issues.push(structure_issue(
            error,
            size,
            origins,
            &mut prepared,
            writer,
        )?);
        Status::Partial
    } else {
        Status::Complete
    };
    let layout = match (chunk_count, family) {
        (None, _) => EntryLayout::InvalidContainer,
        (Some(_), Family::Aim) => EntryLayout::OpaqueAim,
        (Some(_), Family::CafImage | Family::DirectionalBlendImage) => EntryLayout::ImageIndex,
        (Some(_), _) => EntryLayout::ClipInventory,
    };
    let layout_source = if chunk_count.is_some() {
        let mut layout = writer.capture_origins();
        layout.include_original(&extension)?;
        layout.include_original(&origins.payload(
            0..4,
            "containerSignature",
            SourceUseRole::Selector,
            writer,
        )?)?;
        layout.finish()
    } else {
        origins.unknown()
    };
    proof::at(&mut prepared, "/data/payload/layout", layout_source, writer)?;
    let count = if chunk_count.is_some() {
        origins.payload(8..12, "chunkCount", SourceUseRole::Payload, writer)?
    } else {
        origins.unknown()
    };
    proof::at(&mut prepared, "/data/payload/chunk_count", count, writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::ENTRY, Vec::new()),
        status,
        &AnimationPayload::Entry(Entry {
            family,
            layout,
            chunk_count,
        }),
        &issues,
    )
}

fn structure_issue(
    error: &dyn Error,
    size: u64,
    origins: &AnimationOrigins<'_>,
    prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<MediaIssue, MediaError> {
    let evidence = origins.payload(0..size, "animationParse", SourceUseRole::Conversion, writer)?;
    proof::at(prepared, "/issues", evidence, writer)?;
    let detail = error
        .source()
        .map_or_else(|| error.to_string(), |cause| format!("{error}: {cause}"));
    Ok(MediaIssue::new(
        "animation.structure",
        "entry",
        Severity::Warning,
        detail,
    ))
}
