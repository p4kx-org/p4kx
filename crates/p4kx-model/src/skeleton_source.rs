// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{Chunk, ChunkFile, ivo};

use crate::build::corrupt;
use crate::controller_binding::{BoneControllers, ControllerBinder};
use crate::skeleton::{
    COMPACT_HEADER_LEN, COMPACT_RECORD_LEN, ENTRY_LEN, read_bones, read_compact_bones,
};
use crate::{
    Bone, CorruptChunk, ModelBoneSources, ModelError, ModelFile, PrimarySkeletonSelection,
};

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
    const fn measured(chunk: &Chunk) -> Option<Self> {
        match (chunk.chunk_type(), chunk.version()) {
            (ivo::COMPILED_BONES_2, 0x900) => Some(Self::Compiled2),
            (ivo::COMPILED_BONES, 0x901) => Some(Self::Compiled),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Compiled2 => "CompiledBones2",
            Self::Compiled => "CompiledBones",
        }
    }

    fn read(
        self,
        bytes: &[u8],
        sources: Option<&mut ModelBoneSources>,
    ) -> Result<Vec<Bone>, CorruptChunk> {
        match self {
            Self::Compiled2 => read_bones(bytes, sources),
            Self::Compiled => read_compact_bones(bytes, sources),
        }
    }

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
        self.layout
            .read(self.bytes, sources)
            .map_err(corrupt(name, self.index, self.layout.name()))
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
/// One original version-qualified skeleton with entry-relative bone field ranges.
#[derive(Debug)]
pub struct PositionedSkeleton {
    bones: Vec<Bone>,
    sources: ModelBoneSources,
}

impl PositionedSkeleton {
    /// Returns whether the original chunk uses a measured IVO skeleton layout.
    #[must_use]
    pub const fn supports(chunk: &Chunk) -> bool {
        Layout::measured(chunk).is_some()
    }

    /// Reads the bounded bytes of one original skeleton chunk.
    ///
    /// Returns absent for other chunk types or versions.
    ///
    /// # Errors
    /// Returns a positional read error for malformed bytes in a supported skeleton chunk.
    pub fn read(chunk: &Chunk, index: usize, bytes: &[u8]) -> Result<Option<Self>, CorruptChunk> {
        let Some(layout) = Layout::measured(chunk) else {
            return Ok(None);
        };
        let span = usize::try_from(chunk.size())
            .ok()
            .and_then(|size| bytes.get(..size))
            .ok_or_else(|| p4kx_foundation::ParseError::UnexpectedEnd {
                offset: chunk.offset(),
                needed: chunk.size(),
                available: bytes.len() as u64,
            })?;
        let mut sources = ModelBoneSources::new(ModelFile::Model, index, chunk.offset(), None);
        let bones = layout.read(span, Some(&mut sources))?;
        Ok(Some(Self { bones, sources }))
    }

    /// Returns all decoded bones in original table order.
    #[must_use]
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    /// Returns original fields for every decoded bone.
    #[must_use]
    pub const fn sources(&self) -> &ModelBoneSources {
        &self.sources
    }
}

#[cfg(test)]
#[path = "skeleton_source_tests.rs"]
mod tests;
