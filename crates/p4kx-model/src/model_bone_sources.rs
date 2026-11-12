// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::{Bone, ModelFile};

/// Original fields in one physical model skeleton.
#[derive(Debug)]
pub struct ModelBoneSources {
    file: ModelFile,
    chunk: usize,
    pub(crate) start: u64,
    pub(crate) fields: Vec<ModelBoneSource>,
}

impl ModelBoneSources {
    pub(crate) const fn new(file: ModelFile, chunk: usize, start: u64) -> Self {
        Self {
            file,
            chunk,
            start,
            fields: Vec::new(),
        }
    }

    /// Returns the model or level-specific file holding the skeleton.
    #[must_use]
    pub const fn file(&self) -> ModelFile {
        self.file
    }

    /// Returns the original skeleton chunk ordinal.
    #[must_use]
    pub const fn chunk(&self) -> usize {
        self.chunk
    }

    /// Returns the source fields along the actual selected bone's parent route.
    pub fn lineage<'a>(
        &'a self,
        bones: &'a [Bone],
        selected: usize,
    ) -> impl Iterator<Item = (usize, &'a ModelBoneSource)> {
        let mut at = Some(selected);
        (0..bones.len()).map_while(move |_| {
            let index = at?;
            at = bones.get(index)?.parent;
            Some((index, self.fields.get(index)?))
        })
    }
}

/// Original byte intervals for one bone's bind-pose inputs.
#[derive(Debug)]
pub struct ModelBoneSource {
    pub(crate) name: Range<u64>,
    pub(crate) parent: Range<u64>,
    pub(crate) relative: Range<u64>,
}

impl ModelBoneSource {
    /// Returns the name including its stored terminator.
    #[must_use]
    pub fn name(&self) -> Range<u64> {
        self.name.clone()
    }

    /// Returns the stored parent index field.
    #[must_use]
    pub fn parent(&self) -> Range<u64> {
        self.parent.clone()
    }

    /// Returns the stored relative quaternion and translation.
    #[must_use]
    pub fn relative_pose(&self) -> Range<u64> {
        self.relative.clone()
    }
}
