// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ChunkTableRow};

use crate::{Bone, ModelFile};

/// The original source examined by the primary skeleton selection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PrimarySkeletonSelection {
    /// The container's model reader does not expose a skeleton.
    ContainerOnly,
    /// The original type words examined for a skeleton.
    ChunkTable {
        /// The exact examined prefix, absent when the source has no rows.
        prefix: Option<SkeletonTypePrefix>,
    },
}

/// A contiguous sequence of strided type words in the original chunk table.
#[derive(Debug, Clone)]
pub struct SkeletonTypePrefix {
    first: Range<u64>,
    stride: u64,
    count: usize,
}

impl SkeletonTypePrefix {
    /// Returns the half-open first word interval in the decoded Entry.
    #[must_use]
    pub fn first_word(&self) -> Range<u64> {
        self.first.clone()
    }

    /// Returns the byte distance between consecutive words.
    #[must_use]
    pub const fn stride(&self) -> u64 {
        self.stride
    }

    /// Returns the number of examined words.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
}

impl PrimarySkeletonSelection {
    pub(crate) fn chunk_table<B: AsRef<[u8]>>(file: &ChunkFile<B>, count: usize) -> Self {
        let table = file.table_range();
        let stride = (table.end - table.start).checked_div(file.chunks().len() as u64);
        let prefix = file
            .table_row(0)
            .zip(stride)
            .map(|(first, stride)| SkeletonTypePrefix {
                first: first.kind,
                stride,
                count,
            });
        Self::ChunkTable { prefix }
    }
}

/// Original fields in one physical model skeleton.
#[derive(Debug)]
pub struct ModelBoneSources {
    file: ModelFile,
    chunk: usize,
    table_row: Option<ChunkTableRow>,
    pub(crate) start: u64,
    pub(crate) fields: Vec<ModelBoneSource>,
}

impl ModelBoneSources {
    pub(crate) const fn new(
        file: ModelFile,
        chunk: usize,
        start: u64,
        table_row: Option<ChunkTableRow>,
    ) -> Self {
        Self {
            file,
            chunk,
            table_row,
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

    /// Returns the selected chunk's original table row.
    #[must_use]
    pub const fn table_row(&self) -> Option<&ChunkTableRow> {
        self.table_row.as_ref()
    }

    /// Returns the byte interval of the original bone count.
    #[must_use]
    pub const fn count_range(&self) -> Range<u64> {
        self.start..self.start + size_of::<u32>() as u64
    }

    /// Returns the original fields at the decoded bone ordinal.
    #[must_use]
    pub fn bone(&self, ordinal: usize) -> Option<&ModelBoneSource> {
        self.fields.get(ordinal)
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
    pub(crate) controller: Range<u64>,
    pub(crate) node: Option<Range<u64>>,
    pub(crate) relative: Range<u64>,
    pub(crate) world: Range<u64>,
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

    /// Returns the stored controller ID range.
    #[must_use]
    pub fn controller_id(&self) -> Range<u64> {
        self.controller.clone()
    }

    /// Returns the original node word when this layout stores one.
    #[must_use]
    pub fn node(&self) -> Option<Range<u64>> {
        self.node.clone()
    }

    /// Returns the stored model-space quaternion and translation.
    #[must_use]
    pub fn world_pose(&self) -> Range<u64> {
        self.world.clone()
    }

    /// Returns the stored relative quaternion and translation.
    #[must_use]
    pub fn relative_pose(&self) -> Range<u64> {
        self.relative.clone()
    }
}
