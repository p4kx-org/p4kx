// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ivo};

use crate::build::corrupt;
use crate::controller_binding::{BoneControllers, ControllerBinder};
use crate::skeleton::{
    COMPACT_HEADER_LEN, COMPACT_RECORD_LEN, ENTRY_LEN, read_bones, read_compact_bones,
};
use crate::{Bone, ModelBoneSources, ModelError, ModelFile, PrimarySkeletonSelection};

/// An admitted primary skeleton and its original controller table positions.
#[derive(Debug)]
pub struct SkeletonSource {
    controllers: BoneControllers,
    count: usize,
    chunk: usize,
    start: u64,
    layout: Layout,
}

#[derive(Debug, Clone, Copy)]
enum Layout {
    Compiled,
    Compiled2,
}

impl Layout {
    const fn controller_table(self) -> (usize, usize) {
        match self {
            Self::Compiled => (COMPACT_HEADER_LEN, COMPACT_RECORD_LEN),
            Self::Compiled2 => (size_of::<u32>(), ENTRY_LEN),
        }
    }
}

struct SkeletonChunk<'file> {
    index: usize,
    start: u64,
    layout: Layout,
    bytes: &'file [u8],
}

impl<'file> SkeletonChunk<'file> {
    fn primary<B: AsRef<[u8]>>(file: &'file ChunkFile<B>) -> Option<Self> {
        let (layout, (index, chunk)) = [
            (Layout::Compiled2, ivo::COMPILED_BONES_2),
            (Layout::Compiled, ivo::COMPILED_BONES),
        ]
        .into_iter()
        .find_map(|(layout, kind)| Some((layout, file.chunks_of(kind).next()?)))?;
        Some(Self {
            index,
            start: chunk.offset(),
            layout,
            bytes: file.chunk_data(index).unwrap_or_default(),
        })
    }

    fn bones(
        &self,
        name: &str,
        sources: Option<&mut ModelBoneSources>,
    ) -> Result<Vec<Bone>, ModelError> {
        let (chunk_name, bones) = match self.layout {
            Layout::Compiled2 => ("CompiledBones2", read_bones(self.bytes, sources)),
            Layout::Compiled => ("CompiledBones", read_compact_bones(self.bytes, sources)),
        };
        bones.map_err(corrupt(name, self.index, chunk_name))
    }
}

pub(crate) fn read_model_skeleton<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    which: ModelFile,
    read: &mut Vec<(ModelFile, usize)>,
    sources: Option<&mut Vec<ModelBoneSources>>,
    primary_selection: Option<&mut Option<PrimarySkeletonSelection>>,
) -> Result<Vec<Bone>, ModelError> {
    let selected = SkeletonChunk::primary(file);
    if let Some(selection) = primary_selection {
        let examined = match &selected {
            Some(chunk) if matches!(chunk.layout, Layout::Compiled2) => chunk.index + 1,
            _ => file.chunks().len(),
        };
        *selection = Some(PrimarySkeletonSelection::chunk_table(file, examined));
    }
    let Some(chunk) = selected else {
        return Ok(Vec::new());
    };
    read.push((which, chunk.index));
    if let Some(sources) = sources {
        let mut skeleton =
            ModelBoneSources::new(which, chunk.index, chunk.start, file.table_row(chunk.index));
        let bones = chunk.bones(name, Some(&mut skeleton))?;
        sources.push(skeleton);
        Ok(bones)
    } else {
        chunk.bones(name, None)
    }
}

impl SkeletonSource {
    /// Reads the model reader's selected primary skeleton without loading other assets.
    ///
    /// # Errors
    /// Returns a model error if the selected skeleton chunk is invalid.
    pub fn read<B: AsRef<[u8]>>(
        name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Option<Self>, ModelError> {
        let Some(chunk) = SkeletonChunk::primary(file) else {
            return Ok(None);
        };
        let bones = chunk.bones(name, None)?;
        Ok(Some(Self {
            controllers: BoneControllers::new(&bones),
            count: bones.len(),
            chunk: chunk.index,
            start: chunk.start,
            layout: chunk.layout,
        }))
    }

    /// Returns a new cursor for one clip's authored controller order.
    #[must_use]
    pub fn binder(&self) -> ControllerBinder<'_> {
        self.controllers.binder()
    }

    /// Returns the original selected skeleton chunk ordinal.
    #[must_use]
    pub const fn chunk_index(&self) -> usize {
        self.chunk
    }

    /// Returns the count of authored bones.
    #[must_use]
    pub const fn bone_count(&self) -> usize {
        self.count
    }

    /// Returns the source interval of one exact original controller ID.
    #[must_use]
    pub const fn controller_id_range(&self, bone: usize) -> Option<Range<u64>> {
        if bone >= self.count {
            return None;
        }
        let (head, stride) = self.layout.controller_table();
        let at = self.start + (head + bone * stride) as u64;
        Some(at..at + size_of::<u32>() as u64)
    }

    /// Returns the byte interval of the original bone count.
    #[must_use]
    pub const fn bone_count_range(&self) -> Range<u64> {
        self.start..self.start + size_of::<u32>() as u64
    }
}
