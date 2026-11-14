// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::FusedIterator;

use p4kx_chunks::{ChunkFile, ivo};

use crate::animation_dba::{DBA_DATA_VERSION, DBA_SIGNATURE, PREFIX_LEN};
use crate::animation_read::{corrupt, read_block};
use crate::animation_source::SourceChunk;
use crate::{AnimationError, AnimationSource, DbaMetadataRecord, DbaMetadataRecords};

/// A streaming inventory of the clips in a DBA file.
pub struct DbaAnimationSources<'a> {
    file_name: &'a str,
    metadata: DbaMetadataRecords<'a>,
    data: SourceChunk<'a>,
    position: usize,
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
        let metadata = DbaMetadataRecords::chunk(file_name, file)?;
        let data = SourceChunk::read(file_name, file, ivo::DBA_DATA, DBA_DATA_VERSION, "DbaData")?;
        Ok(Self {
            file_name,
            metadata: DbaMetadataRecords::from_chunk(file_name, metadata)?,
            data,
            position: PREFIX_LEN,
            failed: false,
        })
    }

    fn clip(
        &mut self,
        record: DbaMetadataRecord<'a>,
    ) -> Result<AnimationSource<'a>, AnimationError> {
        let block = read_block(self.data.bytes, self.position, DBA_SIGNATURE)
            .map_err(|source| corrupt(self.file_name, "DbaData", source))?;
        let clip = AnimationSource::of(
            record.ordinal,
            record.metadata,
            Some(record.name),
            self.data,
            block,
        );
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
        let record = self.metadata.next()?;
        let clip = record.and_then(|record| self.clip(record));
        self.failed = clip.is_err();
        Some(clip)
    }
}

impl FusedIterator for DbaAnimationSources<'_> {}
