// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{AnimationClipCoordinates, AnimationFrameChannel};
use p4kx_extraction::{
    ArchivePositionedAnimation, FieldOrigin, FieldOrigins, FieldPath, SourceEvidenceError,
    SourceUse, SourceUseRole,
};

use crate::content_origin::EntryPageSources;
use crate::model_animation_sites::{self, ChunkSites, SkeletonSites};
use crate::model_item_page::{self, EvidenceError};

pub(crate) struct ItemUses {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl ItemUses {
    pub(crate) fn of(model: &EntryPageSources) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            uses: vec![model.leaf_path()?],
            complete: model.complete(),
        })
    }

    pub(crate) fn finish(self, model: &EntryPageSources) -> FieldOrigin {
        model.origin_when(&self.uses, self.complete)
    }

    fn origin(
        &mut self,
        entry: &EntryPageSources,
        uses: &[SourceUse],
        complete: bool,
    ) -> FieldOrigin {
        self.uses.extend_from_slice(uses);
        self.complete &= entry.complete() && complete;
        entry.origin_when(uses, complete)
    }
}

pub(crate) struct RowSites<'a> {
    entry: &'a EntryPageSources,
    clip: &'a ArchivePositionedAnimation,
    coords: Option<&'a AnimationClipCoordinates>,
    metadata: Option<ChunkSites>,
    data: Option<ChunkSites>,
}

impl<'a> RowSites<'a> {
    pub(crate) fn new(
        entry: &'a EntryPageSources,
        clip: &'a ArchivePositionedAnimation,
    ) -> Result<Self, SourceEvidenceError> {
        let coords = clip.animation().coordinates();
        let metadata = coords
            .map(|coords| ChunkSites::of(entry.payload_scope(), coords.metadata_chunk()))
            .transpose()?;
        let data = coords
            .map(|coords| ChunkSites::of(entry.payload_scope(), coords.data_chunk()))
            .transpose()?;
        Ok(Self {
            entry,
            clip,
            coords,
            metadata,
            data,
        })
    }

    pub(crate) fn write(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        skeleton: &SkeletonSites,
        aggregate: &mut ItemUses,
    ) -> Result<(), EvidenceError> {
        let source = self.path_origin(aggregate);
        model_item_page::put(fields, path, "source", source.clone())?;
        let Some((coords, metadata, data)) = self
            .coords
            .zip(self.metadata.as_ref())
            .zip(self.data.as_ref())
            .map(|((coords, metadata), data)| (coords, metadata, data))
        else {
            aggregate.complete = false;
            for name in [
                "name",
                "path",
                "frames_per_second",
                "end_frame",
                "additive",
                "duration_seconds",
                "tracks",
                "bound_tracks",
                "keys",
            ] {
                model_item_page::put(fields, path, name, self.entry.origin_when(&[], false))?;
            }
            return Ok(());
        };
        self.write_metadata(fields, path, coords, metadata, source, aggregate)?;
        self.write_counts(fields, path, coords, data, skeleton, aggregate)?;
        model_item_page::put(
            fields,
            path,
            "duration_seconds",
            self.duration(coords, metadata, data, aggregate)?,
        )?;
        Ok(())
    }

    fn path_origin(&self, aggregate: &mut ItemUses) -> FieldOrigin {
        if self
            .clip
            .origin()
            .is_some_and(|origin| origin.entry().path() == self.clip.source_name())
        {
            aggregate.uses.extend_from_slice(&self.entry.path_uses);
            aggregate.complete &= self.entry.path.source_set_complete();
            return self.entry.path.clone();
        }
        aggregate.origin(self.entry, &[], false)
    }

    fn write_metadata(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        coords: &AnimationClipCoordinates,
        selection: &ChunkSites,
        source: FieldOrigin,
        aggregate: &mut ItemUses,
    ) -> Result<(), EvidenceError> {
        let chunk = coords.metadata_chunk().ordinal();
        let name = match coords.database_path() {
            Some(bytes) => aggregate.origin(
                self.entry,
                &self.with_site(
                    selection,
                    chunk,
                    "databasePath",
                    bytes,
                    SourceUseRole::Conversion,
                )?,
                selection.complete,
            ),
            None => source,
        };
        for field in ["name", "path"] {
            model_item_page::put(fields, path, field, name.clone())?;
        }
        for (field, bytes) in [
            ("frames_per_second", coords.frame_rate()),
            ("end_frame", coords.end_frame()),
            ("additive", coords.flags()),
        ] {
            let uses = self.with_site(selection, chunk, field, bytes, SourceUseRole::Conversion)?;
            model_item_page::put(
                fields,
                path,
                field,
                aggregate.origin(self.entry, &uses, selection.complete),
            )?;
        }
        Ok(())
    }

    fn write_counts(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        coords: &AnimationClipCoordinates,
        selection: &ChunkSites,
        skeleton: &SkeletonSites,
        aggregate: &mut ItemUses,
    ) -> Result<(), EvidenceError> {
        let chunk = coords.data_chunk().ordinal();
        let block = coords.controller_block();
        let ids = coords.controller_ids();
        let mut uses = self.with_site(
            selection,
            chunk,
            "controllerBlockHeader",
            block.start..ids.start,
            SourceUseRole::Selector,
        )?;
        let descriptors = ids.end..block.end;
        if !descriptors.is_empty() {
            uses.push(self.site(
                chunk,
                "controllerDescriptors",
                descriptors,
                SourceUseRole::Conversion,
            )?);
        }
        let counts = aggregate.origin(self.entry, &uses, selection.complete);
        model_item_page::put(fields, path, "keys", counts.clone())?;
        model_item_page::put(fields, path, "tracks", counts)?;
        let with_bones = !self.clip.animation().animation().tracks.is_empty();
        if with_bones {
            uses.push(self.site(chunk, "controllerIds", ids, SourceUseRole::Conversion)?);
            uses.extend_from_slice(&skeleton.uses);
        }
        let complete = selection.complete && (!with_bones || skeleton.complete);
        model_item_page::put(
            fields,
            path,
            "bound_tracks",
            aggregate.origin(self.entry, &uses, complete),
        )?;
        Ok(())
    }

    fn duration(
        &self,
        coords: &AnimationClipCoordinates,
        metadata: &ChunkSites,
        data: &ChunkSites,
        aggregate: &mut ItemUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let chunk = coords.metadata_chunk().ordinal();
        let mut uses = self.with_site(
            metadata,
            chunk,
            "frames_per_second",
            coords.frame_rate(),
            SourceUseRole::Conversion,
        )?;
        uses.push(self.site(
            chunk,
            "end_frame",
            coords.end_frame(),
            SourceUseRole::Conversion,
        )?);
        let end = self.clip.animation().animation().end_frame;
        let last = coords
            .last_frame()
            .filter(|frame| u32::from(frame.frame()) > end);
        let mut complete = metadata.complete;
        if let Some(last) = last {
            uses.extend_from_slice(&data.selectors);
            let channel = match last.channel() {
                AnimationFrameChannel::Rotation => "rotation",
                AnimationFrameChannel::Translation => "translation",
                _ => return Ok(aggregate.origin(self.entry, &uses, false)),
            };
            uses.push(model_animation_sites::frame_site(
                self.entry,
                self.clip.animation().clip_ordinal(),
                coords.data_chunk().ordinal(),
                last.controller(),
                channel,
                last.bytes(),
            )?);
            uses.push(self.site(
                coords.data_chunk().ordinal(),
                "frameChannelDescriptor",
                last.descriptor(),
                SourceUseRole::Selector,
            )?);
            complete &= data.complete;
        }
        Ok(aggregate.origin(self.entry, &uses, complete))
    }

    fn with_site(
        &self,
        selection: &ChunkSites,
        chunk: usize,
        field: &str,
        bytes: Range<u64>,
        role: SourceUseRole,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut uses = selection.selectors.clone();
        uses.push(self.site(chunk, field, bytes, role)?);
        Ok(uses)
    }

    fn site(
        &self,
        chunk: usize,
        field: &str,
        bytes: Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        model_animation_sites::clip_site(
            self.entry,
            self.clip.animation().clip_ordinal(),
            chunk,
            field,
            bytes,
            role,
        )
    }
}
