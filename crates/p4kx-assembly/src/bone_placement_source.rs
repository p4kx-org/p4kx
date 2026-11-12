// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntryOrigin, SourceLocation};
use p4kx_model::{Bone, ModelBoneSource, ModelBoneSources, ModelFile};

use crate::model_placement_sources::ModelAnchor;
use crate::placement::BoneSelection;
use crate::{Assembly, Part};

/// Original parent skeleton fields used by one part's bind-pose placement.
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

    /// Returns the original fields from the chosen bone to its root.
    pub fn bones(&self) -> impl Iterator<Item = (usize, &'a ModelBoneSource)> {
        self.fields.lineage(self.skeleton, self.ordinal)
    }
}

impl Assembly {
    /// Returns the observed skeleton used to place this part at a parent model bone.
    #[must_use]
    pub fn bone_placement_source(&self, part: usize) -> Option<BonePlacementSource<'_>> {
        let part = self.parts.get(part)?;
        let ModelAnchor::Bone(selection) = part.model_placement.anchor else {
            return None;
        };
        BonePlacementSource::new(self.parts.get(part.parent()?)?, selection)
    }
}
