// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_archive::EntryOrigin;
use p4kx_gltf::AnimationSummary;
use p4kx_model::AnimationClipCoordinates;

use super::animation_origin_sites::{self, AnimationChunkSites};
use crate::{
    AnimationSet, ArchiveAnimationRead, FieldOrigin, FieldOrigins, FieldPath, InputScope,
    SourceCoordinateOmission, SourceEvidenceError, SourceInput, SourceScope, SourceUseRole,
    SourceUses, directory_path_uses,
};

/// Writes the original clip fields of the standalone GLB command report.
/// The standalone report includes every selected clip in selection order.
///
/// # Errors
/// Returns an error if a selected clip or its original field positions cannot be represented.
pub fn write_glb_animation_origins(
    source: Option<&ArchiveAnimationRead>,
    selected: &AnimationSet,
    written: &[AnimationSummary],
    uses: &SourceUses,
    fields: &mut FieldOrigins,
) -> io::Result<()> {
    if written.len() != selected.animations.len()
        || source.is_some_and(|observed| !std::ptr::eq(observed.set(), selected))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "standalone GLB animation summary differs from its selected clips",
        ));
    }
    let projection = Projection { source, uses };
    let mut path = FieldPath::parse("/animations").map_err(io::Error::other)?;
    for (index, (animation, summary)) in selected.animations.iter().zip(written).enumerate() {
        path.push_index(index).map_err(io::Error::other)?;
        let clip = projection
            .clip(index, animation.end_frame)
            .map_err(io::Error::other)?;
        for (name, origin) in [
            ("name", clip.name),
            ("additive", clip.additive),
            ("duration_seconds", clip.duration_seconds),
        ] {
            put(fields, &path, name, origin)?;
        }
        let baked_fields = ["channels", "keys", "skipped_tracks"];
        let weights = (summary.skipped_weights != 0).then_some("skipped_weights");
        for name in baked_fields.into_iter().chain(weights) {
            put(fields, &path, name, uses.origin(&[], false))?;
        }
        path.pop();
    }
    let mut path = FieldPath::parse("/unread_animations").map_err(io::Error::other)?;
    for (index, row) in selected.unread.iter().enumerate() {
        path.push_index(index).map_err(io::Error::other)?;
        let unread = projection
            .unread(index, row.name.as_deref())
            .map_err(io::Error::other)?;
        for (name, origin) in [
            ("source", unread.source),
            ("name", unread.name),
            ("reason", unread.reason),
        ] {
            put(fields, &path, name, origin)?;
        }
        path.pop();
    }
    Ok(())
}

struct Projection<'a> {
    source: Option<&'a ArchiveAnimationRead>,
    uses: &'a SourceUses,
}

/// The exact original sources of a clip's decoded fields.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ArchiveClipFieldOrigins {
    /// The CAF directory name or original DBA clip name.
    pub name: FieldOrigin,
    /// The original additive flag and its selectors.
    pub additive: FieldOrigin,
    /// The inputs to the decoded duration, including keys that extend its end frame.
    pub duration_seconds: FieldOrigin,
}

impl ArchiveClipFieldOrigins {
    fn unknown(uses: &SourceUses) -> Self {
        let origin = uses.origin(&[], false);
        Self {
            name: origin.clone(),
            additive: origin.clone(),
            duration_seconds: origin,
        }
    }
}

impl ArchiveAnimationRead {
    /// Returns the original field origins for a selected clip.
    /// Missing source evidence yields incomplete fields, and a missing clip yields `None`.
    ///
    /// # Errors
    /// Returns an error when an observed source position cannot be represented.
    pub fn clip_field_origins(
        &self,
        index: usize,
        uses: &SourceUses,
    ) -> Result<Option<ArchiveClipFieldOrigins>, SourceEvidenceError> {
        let Some(clip) = self.set().animations.get(index) else {
            return Ok(None);
        };
        Projection {
            source: Some(self),
            uses,
        }
        .clip(index, clip.end_frame)
        .map(Some)
    }

    /// Returns the original name origin for a selected clip without registering its timing or flags.
    /// Missing source evidence yields an incomplete field, and a missing clip yields `None`.
    ///
    /// # Errors
    /// Returns an error when an observed name or selector position cannot be represented.
    pub fn clip_name_origin(
        &self,
        index: usize,
        uses: &SourceUses,
    ) -> Result<Option<FieldOrigin>, SourceEvidenceError> {
        if self.set().animations.get(index).is_none() {
            return Ok(None);
        }
        let projection = Projection {
            source: Some(self),
            uses,
        };
        let Some(clip) = projection.clip_input(index) else {
            return Ok(Some(uses.origin(&[], false)));
        };
        let payload = projection.payload(clip.entry)?;
        projection.name(&clip, &payload, None).map(Some)
    }
}

struct ClipInput<'clip> {
    entry: &'clip EntryOrigin,
    ordinal: usize,
    coordinates: &'clip AnimationClipCoordinates,
}

struct UnreadFields {
    source: FieldOrigin,
    name: FieldOrigin,
    reason: FieldOrigin,
}

impl Projection<'_> {
    fn clip_input(&self, index: usize) -> Option<ClipInput<'_>> {
        let source = self.source?;
        let entry = source.archive_source(index)?;
        let evidence = source.clip_evidence(index)?;
        Some(ClipInput {
            entry,
            ordinal: evidence.ordinal,
            coordinates: evidence.coordinates.as_ref()?,
        })
    }

    fn clip(
        &self,
        index: usize,
        authored_end: u32,
    ) -> Result<ArchiveClipFieldOrigins, SourceEvidenceError> {
        let Some(clip) = self.clip_input(index) else {
            return Ok(ArchiveClipFieldOrigins::unknown(self.uses));
        };
        let payload = self.payload(clip.entry)?;
        let complete = clip.entry.ancestors().all(|step| step.is_some());
        let coords = clip.coordinates;
        let metadata = AnimationChunkSites::of(&payload, coords.metadata_chunk())?;
        let data = AnimationChunkSites::of(&payload, coords.data_chunk())?;
        let name = self.name(&clip, &payload, Some(&metadata))?;
        let mut additive = metadata.selectors.clone();
        additive.push(animation_origin_sites::clip_site(
            &payload,
            clip.ordinal,
            coords.metadata_chunk().ordinal(),
            "additive",
            coords.flags(),
            SourceUseRole::Conversion,
        )?);
        let duration = animation_origin_sites::duration_sites(
            &payload,
            clip.ordinal,
            coords,
            authored_end,
            &metadata,
            &data,
        )?;
        Ok(ArchiveClipFieldOrigins {
            name,
            additive: self.uses.origin(&additive, metadata.complete && complete),
            duration_seconds: self
                .uses
                .origin(&duration.uses, duration.complete && complete),
        })
    }

    fn name(
        &self,
        clip: &ClipInput<'_>,
        payload: &SourceScope,
        metadata: Option<&AnimationChunkSites>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let Some(bytes) = clip.coordinates.database_path() else {
            return self.path(clip.entry, payload);
        };
        let chunk = clip.coordinates.metadata_chunk();
        let observed;
        let metadata = if let Some(metadata) = metadata {
            metadata
        } else {
            observed = AnimationChunkSites::of(payload, chunk)?;
            &observed
        };
        let mut uses = metadata.selectors.clone();
        uses.push(animation_origin_sites::clip_site(
            payload,
            clip.ordinal,
            chunk.ordinal(),
            "databasePath",
            bytes,
            SourceUseRole::Conversion,
        )?);
        let complete = clip.entry.ancestors().all(|step| step.is_some());
        Ok(self.uses.origin(&uses, metadata.complete && complete))
    }

    fn unread(
        &self,
        index: usize,
        row_name: Option<&str>,
    ) -> Result<UnreadFields, SourceEvidenceError> {
        let unknown = self.uses.origin(&[], false);
        let Some((evidence, entry)) = self
            .source
            .and_then(|source| source.unread_evidence(index))
            .and_then(|evidence| evidence.origin.as_ref().map(|entry| (evidence, entry)))
        else {
            return Ok(UnreadFields {
                source: unknown.clone(),
                name: unknown.clone(),
                reason: unknown,
            });
        };
        let payload = self.payload(entry)?;
        let source = self.path(entry, &payload)?;
        let Some(coordinates) = &evidence.coordinates else {
            if row_name.is_none() {
                payload.at_opaque(
                    SourceUseRole::Attempted,
                    SourceCoordinateOmission::Unavailable,
                )?;
            }
            return Ok(UnreadFields {
                source,
                name: unknown.clone(),
                reason: unknown,
            });
        };
        let metadata = AnimationChunkSites::of(&payload, coordinates.metadata_chunk())?;
        let site = |field, bytes, role| {
            animation_origin_sites::clip_site(
                &payload,
                coordinates.ordinal(),
                coordinates.metadata_chunk().ordinal(),
                field,
                bytes,
                role,
            )
        };
        let name = match coordinates.database_path() {
            Some(bytes) => {
                let mut selectors = metadata.selectors.clone();
                selectors.push(site("databasePath", bytes, SourceUseRole::Conversion)?);
                self.uses.origin(&selectors, false)
            }
            None => source.clone(),
        };
        let reason = match coordinates.additive_flags() {
            Some(bytes) => {
                let mut selectors = metadata.selectors;
                selectors.push(site("additive", bytes, SourceUseRole::Selector)?);
                self.uses.origin(&selectors, false)
            }
            None => unknown,
        };
        Ok(UnreadFields {
            source,
            name,
            reason,
        })
    }

    fn payload(&self, entry: &EntryOrigin) -> Result<SourceScope, SourceEvidenceError> {
        self.uses
            .observe(SourceInput::Archive(entry), InputScope::Payload)
    }

    fn path(
        &self,
        entry: &EntryOrigin,
        payload: &SourceScope,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let directory = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
        let (uses, complete) = directory_path_uses(
            &directory,
            entry.ancestors().len(),
            entry.ancestors().all(|step| step.is_some()),
        )?;
        Ok(self.uses.origin(&uses, complete))
    }
}

fn put(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    field: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let mut path = parent.clone();
    path.push_field(field).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
