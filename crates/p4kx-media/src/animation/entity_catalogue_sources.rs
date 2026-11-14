// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ClipArchiveSource, EntityClipCatalogue};
use p4kx_extraction::animation_headers::AnimationSourceHeader;
use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};

use super::super::{CLIP, Clip, Payload, ReadStatus, SOURCE, Scope, Source};
use super::{Rows, capture, read_status};
use crate::animation::{origin::AnimationOrigins, payload};
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::{
    MediaError, MediaIssue, MediaKind, MediaWriter, ReferenceTarget, Severity, Subresource, source,
};

impl Rows<'_, '_> {
    pub(in super::super::super) fn sources(
        &self,
        catalogue: &EntityClipCatalogue,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (position, row) in catalogue.sources.iter().enumerate() {
            self.source(position, row, writer)?;
        }
        Ok(())
    }

    fn source(
        &self,
        position: usize,
        row: &ClipArchiveSource,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let archive = self.lookup.index.archive;
        let entry = archive
            .entries()
            .get(row.archive_ordinal)
            .ok_or_else(crate::origin_error::foreign)?;
        let input = writer.origins.input(archive, entry)?;
        let parsed = row.file.value();
        write_source(position, row, &input, writer)?;
        if let Some(parsed) = parsed {
            let original = source::entry(self.lookup.root, archive, entry, self.lookup.chain)?;
            for clip in &parsed.clips {
                Self::clip(position, clip, &input, &original, writer)?;
            }
        }
        Ok(())
    }

    fn clip(
        position: usize,
        clip: &AnimationSourceHeader,
        input: &MediaOriginInput,
        original: &crate::Source,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut scope = Scope::new(writer);
        let physical_record = clip_reference(clip, original);
        clip_proof(&mut scope, clip, input, physical_record.as_ref(), writer)?;
        let timing = clip.timing.as_ref().ok();
        scope.write(
            CLIP,
            vec![position as u64, clip.ordinal as u64],
            &Payload::Clip(Clip {
                metadata_range: payload::entry_range(clip.metadata_range.clone()),
                metadata_version: clip.metadata_version,
                flags: clip.flags,
                frames_per_second: timing.map(|value| value.frames_per_second),
                end_frame: timing.map(|value| value.end_frame),
                name_bytes: clip
                    .database_name
                    .as_ref()
                    .map(|name| name.bytes.as_slice()),
                controller_count: clip
                    .controller_block
                    .as_ref()
                    .map(|block| block.controllers.len()),
                physical_record,
            }),
            &[],
            writer,
        )
    }
}

fn write_source(
    position: usize,
    row: &ClipArchiveSource,
    input: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let parsed = row.file.value();
    let mut scope = Scope::new(writer);
    for field in ["entry", "path", "directory_ordinal"] {
        let proof = capture(
            input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[row.archive_ordinal as u64], field),
            writer,
        )?;
        scope.at(field, proof, writer)?;
    }
    let status = if parsed.is_some_and(|headers| headers.partial.is_some()) {
        ReadStatus::Partial
    } else {
        read_status(&row.file)
    };
    if let Some(origin) = row.file.source() {
        let proof = capture(
            input,
            InputScope::Payload,
            OriginSite::new(
                "animation-source",
                &[origin.entry().index() as u64],
                "headerRead",
            ),
            writer,
        )?;
        scope.at("status", proof.clone(), writer)?;
        if !matches!(status, ReadStatus::Read) {
            scope.health(proof, writer)?;
        }
    }
    let issue = match status {
        ReadStatus::Read | ReadStatus::NotSelected => None,
        _ => Some(MediaIssue::new(
            "animation.entity-source",
            "source",
            Severity::Warning,
            "the selected animation source headers are incomplete",
        )),
    };
    scope.write(
        SOURCE,
        vec![position as u64],
        &Payload::Source(Source {
            entry: row.archive_ordinal,
            directory_ordinal: row.directory_ordinal.map(DecimalU64::new),
            path: &row.file.path,
            status,
        }),
        issue.as_slice(),
        writer,
    )
}

fn clip_reference(
    clip: &AnimationSourceHeader,
    original: &crate::Source,
) -> Option<ReferenceTarget> {
    let namespace = if clip.controller_block.is_some() {
        Some(payload::CLIP)
    } else {
        clip.database_name.as_ref().map(|_| payload::DBA_METADATA)
    };
    namespace.map(|namespace| {
        let key = Subresource::new(namespace, vec![clip.ordinal as u64]);
        ReferenceTarget::new_with_subresource(original, MediaKind::Animation, key)
    })
}

fn clip_proof(
    scope: &mut Scope,
    clip: &AnimationSourceHeader,
    input: &MediaOriginInput,
    target: Option<&ReferenceTarget>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = clip.metadata_range.clone();
    let origins = AnimationOrigins {
        input,
        entry: clip.metadata_chunk,
    };
    for (member, bytes) in [
        ("metadata_range", range.clone()),
        ("flags", range.start..range.start + 4),
        ("frames_per_second", range.start + 4..range.start + 6),
        ("end_frame", range.start + 16..range.start + 20),
    ] {
        let proof = origins.payload(bytes, member, SourceUseRole::Payload, writer)?;
        scope.at(member, proof, writer)?;
    }
    let metadata_locator = [clip.metadata_chunk as u64];
    scope.at(
        "metadata_version",
        capture(
            input,
            InputScope::Payload,
            OriginSite::new("animation-chunk-table", &metadata_locator, "version"),
            writer,
        )?,
        writer,
    )?;
    if let Some(name) = &clip.database_name {
        let proof = origins.payload(
            name.range.clone(),
            "databaseName",
            SourceUseRole::Payload,
            writer,
        )?;
        scope.at("name_bytes", proof, writer)?;
    }
    if let Some(block) = &clip.controller_block {
        scope.at(
            "controller_count",
            origins.payload(
                block.range.clone(),
                "controllerTable",
                SourceUseRole::Payload,
                writer,
            )?,
            writer,
        )?;
    }
    if let Some(target) = target {
        let proof = origins.payload(
            clip.metadata_range.clone(),
            "clipIdentity",
            SourceUseRole::Selector,
            writer,
        )?;
        scope.clip_reference(target, input, proof, writer)?;
    }
    Ok(())
}
