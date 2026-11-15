// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{EntryOrigin, SourceLocation};
use p4kx_model::{
    AttachmentBoneBinding, Bone, ModelBoneSource, ModelBoneSources, ModelFile,
    resolve_attachment_bone,
};

use crate::model_placement_sources::{BoneOwner, ModelAnchor};
use crate::placement::BoneSelection;
use crate::{Assembly, Part};

/// Original skeleton fields used by one part's bind-pose placement.
#[derive(Debug)]
pub struct BonePlacementSource<'a> {
    owner: &'a Part,
    origin: &'a EntryOrigin,
    fields: &'a ModelBoneSources,
    skeleton: &'a [Bone],
    ordinal: usize,
}

impl<'a> BonePlacementSource<'a> {
    fn new(owner: &'a Part, selection: BoneSelection) -> Option<Self> {
        let model = owner.model()?;
        let skeleton = match selection.file {
            ModelFile::Model => &model.bones,
            ModelFile::LodModel(level) => model
                .lods
                .iter()
                .find(|lod| lod.level == level)?
                .bones
                .as_ref()?,
            _ => return None,
        };
        let source = owner
            .model_archive_sources()
            .iter()
            .find(|source| source.file() == selection.file)?;
        Some(Self {
            owner,
            origin: source.origin()?,
            fields: source.bone_sources()?,
            skeleton,
            ordinal: selection.ordinal,
        })
    }

    /// Returns the actual Archive Entry supplying the selected bind pose.
    #[must_use]
    pub const fn origin(&self) -> &'a EntryOrigin {
        self.origin
    }

    /// Returns the authored selector for the model supplying this skeleton.
    #[must_use]
    pub fn geometry_selector(&self) -> Option<SourceLocation> {
        self.owner.geometry_selector()
    }

    /// Returns the original CDF attributes that selected the model supplying this placement.
    pub fn cdf_model_selectors(&self) -> impl Iterator<Item = SourceLocation> {
        self.owner.cdf_model_selectors()
    }

    /// Returns the physical character definition that selected the model.
    #[must_use]
    pub const fn definition_origin(&self) -> Option<&EntryOrigin> {
        self.owner.model_definition_origin()
    }

    /// Returns the physical skeleton chunk ordinal.
    #[must_use]
    pub const fn chunk(&self) -> usize {
        self.fields.chunk()
    }

    /// Returns which model file supplied the selected bone.
    #[must_use]
    pub const fn file(&self) -> ModelFile {
        self.fields.file()
    }

    /// Returns the original selected bone ordinal.
    #[must_use]
    pub const fn bone_ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the original count of physical bones in this skeleton.
    #[must_use]
    pub const fn bone_count_range(&self) -> Range<u64> {
        self.fields.count_range()
    }

    /// Returns the complete original name span for this skeleton's physical bones.
    #[must_use]
    pub fn bone_name_table_range(&self) -> Option<Range<u64>> {
        let count = self.skeleton.len();
        if self.fields.bone(count).is_some() {
            return None;
        }
        let first = self.fields.bone(0)?.name();
        let last = self.fields.bone(count.checked_sub(1)?)?.name();
        Some(first.start..last.end)
    }

    /// Returns the original fields from the chosen bone to its root.
    pub fn bones(&self) -> impl Iterator<Item = (usize, &'a ModelBoneSource)> {
        self.fields.lineage(self.skeleton, self.ordinal)
    }
}

impl Assembly {
    /// Returns the parent or character-root skeleton used to place this part.
    #[must_use]
    pub fn bone_placement_source(&self, part: usize) -> Option<BonePlacementSource<'_>> {
        let part = self.parts.get(part)?;
        let ModelAnchor::Bone { owner, selection } = part.model_placement.anchor else {
            return None;
        };
        let owner = match owner {
            BoneOwner::Parent => self.parts.get(part.parent()?)?,
            BoneOwner::Root => self.parts.first()?,
        };
        BonePlacementSource::new(owner, selection)
    }
}

impl Part {
    /// Returns the original skeleton used by this part's own CDF bone attachment.
    #[must_use]
    pub fn attachment_bone_placement_source(
        &self,
        ordinal: usize,
    ) -> Option<BonePlacementSource<'_>> {
        BonePlacementSource::new(self, self.attachments.get(ordinal)?.bone_selection?)
    }

    /// Returns original fields when a unique, fully sourced bone establishes the attachment frame.
    #[must_use]
    pub fn verified_attachment_bone_placement_source(
        &self,
        ordinal: usize,
    ) -> Option<BonePlacementSource<'_>> {
        let attachment = self.attachments.get(ordinal)?;
        let source = BonePlacementSource::new(self, attachment.bone_selection?)?;
        let AttachmentBoneBinding::Bound {
            ordinal, lineage, ..
        } = resolve_attachment_bone(source.skeleton, attachment.bone())
        else {
            return None;
        };
        (ordinal == source.ordinal
            && source.bone_name_table_range().is_some()
            && lineage
                .into_iter()
                .all(|at| source.fields.bone(at).is_some()))
        .then_some(source)
    }
}
