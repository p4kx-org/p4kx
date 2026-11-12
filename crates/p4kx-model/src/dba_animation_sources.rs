// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::{Enumerate, FusedIterator};
use std::slice::Iter;

use p4kx_chunks::{ChunkFile, ivo};

use crate::animation_dba::{
    DBA_DATA_VERSION, DBA_METADATA_VERSION, DBA_SIGNATURE, METADATA_HEADER_LEN, PREFIX_LEN,
    metadata_parts, next_name,
};
use crate::animation_read::{RECORD_LEN, corrupt, read_block};
use crate::animation_source::SourceChunk;
use crate::{AnimationError, AnimationMetadata, AnimationSource, DbaAnimationName};

/// A streaming inventory of the clips in a DBA file.
pub struct DbaAnimationSources<'a> {
    file_name: &'a str,
    metadata: SourceChunk<'a>,
    records: Enumerate<Iter<'a, [u8; RECORD_LEN]>>,
    count: usize,
    data: SourceChunk<'a>,
    position: usize,
    names: &'a [u8],
    name_at: u64,
    failed: bool,
}

impl<'a> DbaAnimationSources<'a> {
    /// Opens the metadata and controller data of a DBA file.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, unsupported or truncated chunk structure.
    pub fn read<B: AsRef<[u8]>>(
        file_name: &'a str,
        file: &'a ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        let metadata = SourceChunk::read(
            file_name,
            file,
            ivo::DBA_METADATA,
            DBA_METADATA_VERSION,
            "DbaMetadata",
        )?;
        let data = SourceChunk::read(file_name, file, ivo::DBA_DATA, DBA_DATA_VERSION, "DbaData")?;
        let (records, names) = metadata_parts(metadata.bytes)
            .map_err(|source| corrupt(file_name, "DbaMetadata", source))?;
        let name_at = metadata.at + METADATA_HEADER_LEN as u64 + records.len() as u64;
        let records = records.as_chunks::<RECORD_LEN>().0;
        Ok(Self {
            file_name,
            metadata,
            records: records.iter().enumerate(),
            count: records.len(),
            data,
            position: PREFIX_LEN,
            names,
            name_at,
            failed: false,
        })
    }

    fn clip(
        &mut self,
        ordinal: usize,
        record: &'a [u8; RECORD_LEN],
    ) -> Result<AnimationSource<'a>, AnimationError> {
        let (name, remaining) = next_name(self.names, self.count)
            .map_err(|source| corrupt(self.file_name, "DbaMetadata", source))?;
        let block = read_block(self.data.bytes, self.position, DBA_SIGNATURE)
            .map_err(|source| corrupt(self.file_name, "DbaData", source))?;
        let record_at =
            self.metadata.at + METADATA_HEADER_LEN as u64 + ordinal as u64 * RECORD_LEN as u64;
        let clip = AnimationSource::of(
            ordinal,
            AnimationMetadata::of(
                self.metadata.index,
                record_at,
                self.metadata.version,
                record,
            ),
            Some(DbaAnimationName::of(self.name_at, name)),
            self.data,
            block,
        );
        self.names = remaining;
        self.name_at += name.len() as u64 + 1;
        self.position = block.end;
        Ok(clip)
    }
}

impl<'a> Iterator for DbaAnimationSources<'a> {
    type Item = Result<AnimationSource<'a>, AnimationError>;

    /// Advances one clip and stops after a structural error.
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let (ordinal, record) = self.records.next()?;
        let clip = self.clip(ordinal, record);
        self.failed = clip.is_err();
        Some(clip)
    }
}

impl FusedIterator for DbaAnimationSources<'_> {}
